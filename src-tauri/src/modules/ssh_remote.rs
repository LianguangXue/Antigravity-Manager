//! Remote SSH account sync.
//!
//! Antigravity Remote-SSH runs a language server on the SSH host, and that
//! server keeps its own credential file (`~/.gemini/jetski-standalone-oauth-token`).
//! A local account switch therefore never reaches remote windows. When enabled,
//! every switch also:
//!
//! 1. backs up the remote credential file,
//! 2. atomically writes the selected account's credential,
//! 3. stops the remote language server so the client can start a fresh one.
//!
//! Related: #3886, #3887, #525.

use crate::models::config::SshRemoteConfig;
use crate::modules::logger;
use std::io::Write;
use std::process::{Command, Stdio};

const REMOTE_TOKEN_PATH: &str = "~/.gemini/jetski-standalone-oauth-token";
const REMOTE_KILL_CMD: &str =
    "pkill -f '[a]ntigravity-hub-server/bin/.*/language_server' >/dev/null 2>&1 || true; \
pkill -f '[a]gy --hub' >/dev/null 2>&1 || true; echo STOPPED";

fn run_ssh(host: &str, remote_cmd: &str, stdin_data: Option<String>) -> Result<String, String> {
    let mut cmd = Command::new("ssh");
    cmd.args([
        "-o",
        "BatchMode=yes",
        "-o",
        "ConnectTimeout=10",
        "-o",
        "StrictHostKeyChecking=accept-new",
        host,
        remote_cmd,
    ]);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: never flash a console window from the GUI app.
        cmd.creation_flags(0x0800_0000);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("failed to launch ssh for {}: {}", host, e))?;

    if let Some(data) = stdin_data {
        if let Some(stdin) = child.stdin.as_mut() {
            stdin
                .write_all(data.as_bytes())
                .map_err(|e| format!("failed to write ssh stdin for {}: {}", host, e))?;
        }
    }
    drop(child.stdin.take());

    let output = child
        .wait_with_output()
        .map_err(|e| format!("failed to wait for ssh {}: {}", host, e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "ssh {} failed: {} {}",
            host,
            stderr.trim(),
            stdout.trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Return the SSH host aliases configured in `~/.ssh/config`.
pub fn list_ssh_hosts() -> Vec<String> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let config_path = home.join(".ssh").join("config");
    let Ok(content) = std::fs::read_to_string(&config_path) else {
        return Vec::new();
    };

    let mut hosts: Vec<String> = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let Some(keyword) = parts.next() else {
            continue;
        };
        if !keyword.eq_ignore_ascii_case("host") {
            continue;
        }
        for candidate in parts {
            // Wildcards and negations are not concrete connectable hosts.
            if candidate.contains('*') || candidate.contains('?') || candidate.starts_with('!') {
                continue;
            }
            if !hosts.iter().any(|h| h.as_str() == candidate) {
                hosts.push(candidate.to_string());
            }
        }
    }
    hosts
}

/// Verify that the given SSH host can be reached non-interactively.
pub async fn test_host(host: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        run_ssh(&host, "echo SSH_REMOTE_OK", None).map(|out| out.trim().to_string())
    })
    .await
    .map_err(|e| format!("ssh test task failed: {}", e))?
}

fn push_credential(host: &str, credential_json: &str) -> Result<(), String> {
    let push_cmd = format!(
        "set -e; mkdir -p ~/.gemini; \
if [ -f {path} ]; then cp -f {path} {path}.bak.$(date +%s); fi; \
cat > {path}.tmp; \
mv -f {path}.tmp {path}; \
chmod 600 {path}; \
echo PUSHED",
        path = REMOTE_TOKEN_PATH
    );
    run_ssh(host, &push_cmd, Some(credential_json.to_string()))?;
    Ok(())
}

fn stop_remote_server(host: &str) -> Result<(), String> {
    run_ssh(host, REMOTE_KILL_CMD, None)?;
    Ok(())
}

/// Restart the local Antigravity client so its SSH windows reconnect and start
/// remote servers that read the freshly pushed credential.
///
/// The client is closed first (waiting until it is really gone, otherwise the
/// new instance exits on the single-instance lock) and then started through the
/// user's proxy launcher when present, so the proxy flags survive the restart.
pub async fn restart_local_client() {
    let running = tokio::task::spawn_blocking(|| {
        crate::modules::process::is_antigravity_running(Some("classic"))
    })
    .await
    .unwrap_or(false);

    if running {
        let closed = tokio::task::spawn_blocking(|| {
            crate::modules::process::close_antigravity(8, Some("classic"))
        })
        .await
        .unwrap_or_else(|e| Err(format!("restart task failed: {}", e)));

        if let Err(e) = closed {
            logger::log_warn(&format!(
                "[SSH Remote] close Antigravity returned an error (continuing): {}",
                e
            ));
        }
    }

    // Wait until the old process is really gone; a fast restart loses the
    // single-instance lock race and the new instance exits immediately.
    for _ in 0..12 {
        let still_running = tokio::task::spawn_blocking(|| {
            crate::modules::process::is_antigravity_running(Some("classic"))
        })
        .await
        .unwrap_or(false);
        if !still_running {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(700)).await;
    }

    // Prefer the user's proxy launcher so `--proxy-server` is applied again.
    let launcher = dirs::data_local_dir()
        .map(|dir| {
            dir.join("AntigravityProxy")
                .join("launch-antigravity-proxy.vbs")
        })
        .filter(|path| path.is_file());

    if let Some(launcher) = launcher {
        let started = tokio::task::spawn_blocking(move || {
            std::process::Command::new("wscript.exe")
                .arg("//nologo")
                .arg(&launcher)
                .spawn()
                .map(|_| ())
                .map_err(|e| format!("failed to launch proxy launcher: {}", e))
        })
        .await
        .unwrap_or_else(|e| Err(format!("restart task failed: {}", e)));

        if let Err(e) = started {
            logger::log_warn(&format!("[SSH Remote] Proxy launcher failed: {}", e));
        }
    } else {
        let started = tokio::task::spawn_blocking(|| {
            crate::modules::process::start_antigravity(Some("classic"))
        })
        .await
        .unwrap_or_else(|e| Err(format!("restart task failed: {}", e)));

        if let Err(e) = started {
            logger::log_warn(&format!(
                "[SSH Remote] Failed to start Antigravity client after switch: {}",
                e
            ));
            return;
        }
    }

    // Give the client time to boot; log if it never comes up.
    for _ in 0..14 {
        let up = tokio::task::spawn_blocking(|| {
            crate::modules::process::is_antigravity_running(Some("classic"))
        })
        .await
        .unwrap_or(false);
        if up {
            logger::log_info(
                "[SSH Remote] Antigravity client restarted; SSH windows will reconnect.",
            );
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(700)).await;
    }

    logger::log_warn("[SSH Remote] Antigravity client did not come back up after the switch.");
}

/// Push the account credential to every configured SSH host and stop its server.
///
/// Returns one entry per host so callers can log a per-host result.
pub async fn sync_after_switch(
    account: &crate::models::Account,
    config: &SshRemoteConfig,
) -> Vec<(String, Result<(), String>)> {
    if !config.enabled || config.hosts.is_empty() {
        return Vec::new();
    }

    let credential_json = match crate::modules::integration::build_file_token_json(account) {
        Ok(json) => json,
        Err(e) => {
            logger::log_warn(&format!(
                "[SSH Remote] Failed to build file credential: {}",
                e
            ));
            return vec![("*".to_string(), Err(e))];
        }
    };

    let mut results = Vec::new();
    for host in &config.hosts {
        let host = host.trim().to_string();
        if host.is_empty() {
            continue;
        }

        let credential = credential_json.clone();
        let target = host.clone();
        let result = tokio::task::spawn_blocking(move || {
            push_credential(&target, &credential)?;
            stop_remote_server(&target)
        })
        .await
        .unwrap_or_else(|e| Err(format!("ssh sync task failed: {}", e)));

        match &result {
            Ok(()) => logger::log_info(&format!(
                "[SSH Remote] Credential pushed and language server stopped on {} for {}",
                host, account.email
            )),
            Err(e) => logger::log_warn(&format!("[SSH Remote] Sync failed on {}: {}", host, e)),
        }

        results.push((host, result));
    }

    results
}

#[cfg(test)]
mod tests {
    use super::list_ssh_hosts;

    #[test]
    fn host_listing_does_not_panic_without_config() {
        // Must never panic; an empty list is a valid result on a fresh machine.
        let _ = list_ssh_hosts();
    }
}
