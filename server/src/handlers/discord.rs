use serde_json::{json, Value};

use crate::state::AppState;
use crate::util::get_arg;

pub(crate) async fn handle(state: &AppState, command: &str, payload: &Value) -> Result<Value, String> {
    match command {
        "discord_test_webhook" => {
            log::info!("[discord_test_webhook] Received payload: {:?}", payload);
            let url: String = get_arg(payload, "url")?;
            log::info!("[discord_test_webhook] Testing URL: {}", if url.len() > 50 { &url[..50] } else { &url });
            let result = state.discord_service.test_webhook(&url).await;
            log::info!("[discord_test_webhook] Result: {:?}", result);
            Ok(json!(result))
        }
        "discord_send_notification" => {
            let profile_settings = state
                .active_profile_settings
                .lock()
                .await
                .clone()
                .ok_or_else(|| "No active profile loaded".to_string())?;
            let discord_settings = profile_settings.discord;
            if !discord_settings.webhook_enabled {
                return Ok(json!({
                    "success": false,
                    "message": "Discord webhook is not enabled",
                    "skippedCooldown": false
                }));
            }
            let image_path = if discord_settings.image_path.is_empty() {
                None
            } else {
                Some(discord_settings.image_path.as_str())
            };
            let result = state.discord_service.send_go_live_notification(
                &discord_settings.webhook_url,
                &discord_settings.go_live_message,
                image_path,
                discord_settings.cooldown_enabled,
                discord_settings.cooldown_seconds,
            ).await;
            Ok(json!(result))
        }
        "discord_reset_cooldown" => {
            state.discord_service.reset_cooldown().await;
            Ok(Value::Null)
        }
        _ => Err(format!("Unknown discord command: {command}")),
    }
}
