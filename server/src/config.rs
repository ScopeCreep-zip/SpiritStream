use std::env;
use std::path::PathBuf;

use crate::util::{find_themes_dir_fallback, parse_bool};
use spiritstream_server::models::ProfileSettings;
use spiritstream_server::services::{ProfileManager, SettingsManager};

/// All resolved configuration for the server, derived from env vars + settings.
pub(crate) struct ServerConfig {
    pub app_data_dir: PathBuf,
    pub log_dir: PathBuf,
    pub themes_dir: String,
    pub ui_dir: String,
    pub ui_enabled: bool,
    pub auth_token: Option<String>,
    pub host: String,
    pub port: u16,
    pub is_localhost: bool,
    pub rate_limit: u32,
}

impl ServerConfig {
    pub async fn from_env(
        profile_manager: &ProfileManager,
        settings_manager: &SettingsManager,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Load configuration from environment
        let data_dir = env::var("SPIRITSTREAM_DATA_DIR").unwrap_or_else(|_| "data".to_string());
        let log_dir = env::var("SPIRITSTREAM_LOG_DIR")
            .unwrap_or_else(|_| format!("{data_dir}/logs"));
        // Resolve themes directory with fallback logic
        let themes_dir = match env::var("SPIRITSTREAM_THEMES_DIR") {
            Ok(dir) => {
                let path = PathBuf::from(&dir);
                let has_themes = std::fs::read_dir(&path)
                    .map(|entries| {
                        entries.flatten().any(|e| {
                            e.path()
                                .extension()
                                .map(|ext| ext == "jsonc" || ext == "json")
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false);

                if has_themes {
                    dir
                } else if let Some(fallback) = find_themes_dir_fallback() {
                    fallback
                } else {
                    dir
                }
            }
            Err(_) => {
                if let Some(fallback) = find_themes_dir_fallback() {
                    fallback
                } else {
                    "themes".to_string()
                }
            }
        };
        let ui_dir = env::var("SPIRITSTREAM_UI_DIR").unwrap_or_else(|_| "dist".to_string());
        let env_host = env::var("SPIRITSTREAM_HOST").ok();
        let env_port: Option<u16> = env::var("SPIRITSTREAM_PORT")
            .ok()
            .and_then(|value| value.parse().ok());
        let env_auth_token = env::var("SPIRITSTREAM_API_TOKEN")
            .or_else(|_| env::var("SPIRITSTREAM_DEV_TOKEN"))
            .ok()
            .and_then(|value| {
                let trimmed = value.trim().to_string();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            });

        let app_data_dir = PathBuf::from(&data_dir);
        let log_dir_path = PathBuf::from(&log_dir);
        std::fs::create_dir_all(&app_data_dir)?;
        std::fs::create_dir_all(&log_dir_path)?;

        // Load settings (global)
        let settings = settings_manager.load().ok();
        let last_profile = settings
            .as_ref()
            .and_then(|s| s.last_profile.clone());

        // Load backend settings from the last active profile (per-profile integration)
        let mut backend_settings = ProfileSettings::default().backend;
        if let Some(profile_name) = last_profile.as_deref() {
            match profile_manager.load_with_key_decryption(profile_name, None).await {
                Ok(profile) => {
                    backend_settings = profile.settings.backend;
                }
                Err(err) => {
                    log::warn!(
                        "Failed to load backend settings from profile '{}': {err}",
                        profile_name
                    );
                }
            }
        }

        let settings_ui_enabled = backend_settings.ui_enabled;
        let env_ui_enabled = env::var("SPIRITSTREAM_UI_ENABLED")
            .ok()
            .and_then(|value| parse_bool(&value));
        let ui_enabled = env_ui_enabled.unwrap_or(settings_ui_enabled);
        let settings_auth_token = {
            let trimmed = backend_settings.token.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        };
        let auth_token = env_auth_token.or(settings_auth_token);

        // Determine host/port: env vars take precedence, then settings, then defaults
        let (host, port) = {
            let remote_enabled = backend_settings.remote_enabled;
            let settings_host = backend_settings.host.clone();
            let settings_port = backend_settings.port;

            let env_host_was_set = env_host.is_some();

            let configured_host = env_host.unwrap_or(settings_host);
            let configured_port = env_port.unwrap_or(settings_port);

            let final_host = if !remote_enabled && !env_host_was_set {
                spiritstream_server::constants::DEFAULT_HOST.to_string()
            } else {
                configured_host
            };

            (final_host, configured_port)
        };
        let is_localhost = crate::util::parse_host(&host).is_loopback();

        let rate_limit = env::var("SPIRITSTREAM_RATE_LIMIT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(spiritstream_server::constants::DEFAULT_RATE_LIMIT_PER_MINUTE);

        Ok(ServerConfig {
            app_data_dir,
            log_dir: log_dir_path,
            themes_dir,
            ui_dir,
            ui_enabled,
            auth_token,
            host,
            port,
            is_localhost,
            rate_limit,
        })
    }
}
