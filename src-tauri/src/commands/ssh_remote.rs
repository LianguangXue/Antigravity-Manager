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
