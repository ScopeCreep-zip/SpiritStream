use serde_json::{json, Value};
use std::path::PathBuf;

use crate::state::AppState;
use crate::util::get_arg;
use spiritstream_server::services::validate_extension;

pub(crate) async fn handle(state: &AppState, command: &str, payload: &Value) -> Result<Value, String> {
    match command {
        "list_themes" => {
            let themes = state.theme_manager.list_themes();
            log::info!(
                "[THEME CMD] list_themes returning {} themes: {:?}",
                themes.len(),
                themes.iter().map(|t| &t.id).collect::<Vec<_>>()
            );
            Ok(json!(themes))
        }
        "refresh_themes" => {
            log::info!("[THEME CMD] refresh_themes called");
            state.theme_manager.sync_project_themes();
            let themes = state.theme_manager.list_themes();
            log::info!(
                "[THEME CMD] refresh_themes returning {} themes",
                themes.len()
            );
            Ok(json!(themes))
        }
        "get_theme_tokens" => {
            let theme_id: String = get_arg(payload, "themeId")?;
            log::info!("[THEME CMD] get_theme_tokens called for: {theme_id}");
            match state.theme_manager.get_theme_tokens(&theme_id) {
                Ok(tokens) => {
                    log::info!(
                        "[THEME CMD] Returning {} tokens for {}",
                        tokens.len(),
                        theme_id
                    );
                    // Log sample tokens for verification
                    for (i, (key, value)) in tokens.iter().take(3).enumerate() {
                        log::info!("[THEME CMD] Sample token {}: {} = {}", i + 1, key, value);
                    }
                    Ok(json!(tokens))
                }
                Err(e) => {
                    log::error!("[THEME CMD] get_theme_tokens failed for {theme_id}: {e}");
                    Err(e)
                }
            }
        }
        "install_theme" => {
            let theme_path: String = get_arg(payload, "themePath")?;
            let path = PathBuf::from(&theme_path);

            // Security: Validate file extension
            validate_extension(&path, &["json", "jsonc"])?;

            // Security: Reject paths with traversal sequences
            if theme_path.contains("..") {
                return Err("Invalid theme path: path traversal not allowed".to_string());
            }

            Ok(json!(state.theme_manager.install_theme(&path)?))
        }
        _ => Err(format!("Unknown themes command: {command}")),
    }
}
