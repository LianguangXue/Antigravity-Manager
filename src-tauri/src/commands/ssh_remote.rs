use crate::models::config::SshRemoteConfig;

/// Read the remote SSH sync configuration.
#[tauri::command]
pub fn get_ssh_remote_config() -> Result<SshRemoteConfig, String> {
    Ok(crate::modules::config::load_app_config()?.ssh_remote)
}

/// Persist the remote SSH sync configuration.
#[tauri::command]
pub fn save_ssh_remote_config(config: SshRemoteConfig) -> Result<(), String> {
    let mut app_config = crate::modules::config::load_app_config()?;
    app_config.ssh_remote = config;
    crate::modules::config::save_app_config(&app_config)
}

/// List SSH host aliases from `~/.ssh/config`.
#[tauri::command]
pub fn list_ssh_remote_hosts() -> Vec<String> {
    crate::modules::ssh_remote::list_ssh_hosts()
}

/// Test a single SSH host non-interactively.
#[tauri::command]
pub async fn test_ssh_remote_host(host: String) -> Result<String, String> {
    crate::modules::ssh_remote::test_host(host).await
}

/// Switch only the configured remote SSH hosts to the given account, leaving the
/// local Antigravity account untouched. Related: #3886, #3887, #525.
#[tauri::command]
pub async fn switch_ssh_only(account_id: String) -> Result<String, String> {
    let mut account = crate::modules::load_account(&account_id)?;
    let config = crate::modules::config::load_app_config()?.ssh_remote;
    if !config.enabled || config.hosts.is_empty() {
        return Err(
            "Remote SSH sync is not enabled. Enable it in Settings > Remote SSH Sync.".to_string(),
        );
    }

    let fresh = crate::modules::oauth::ensure_fresh_token(&account.token, Some(&account.id)).await?;
    if fresh.access_token != account.token.access_token {
        account.token = fresh;
        let _ = crate::modules::save_account(&account);
    }

    let results = crate::modules::ssh_remote::sync_after_switch(&account, &config).await;
    let mut switched_hosts = Vec::new();
    let mut errors = Vec::new();
    for (host, result) in results {
        match result {
            Ok(()) => switched_hosts.push(host),
            Err(e) => errors.push(format!("{}: {}", host, e)),
        }
    }

    if switched_hosts.is_empty() {
        return Err(format!("Remote SSH switch failed: {}", errors.join("; ")));
    }

    if config.restart_client {
        crate::modules::ssh_remote::restart_local_client().await;
    }

    let mut message = format!(
        "Remote SSH switched to {} on {}",
        account.email,
        switched_hosts.join(", ")
    );
    if !errors.is_empty() {
        message.push_str(&format!(" (failed: {})", errors.join("; ")));
    }
    Ok(message)
}
