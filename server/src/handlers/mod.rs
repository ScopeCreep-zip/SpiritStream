mod chat;
mod discord;
mod ffmpeg;
mod obs;
mod oauth;
mod profiles;
mod settings;
mod streaming;
mod themes;

use serde_json::Value;

use spiritstream_server::constants::QUIET_COMMANDS;
use crate::security::redaction::redact_payload;
use crate::state::AppState;

/// Dispatch a command to the appropriate domain handler.
/// This is the central router for all `POST /api/invoke/:command` requests.
pub(crate) async fn dispatch(
    state: &AppState,
    command: &str,
    payload: Value,
) -> Result<Value, String> {
    // Only log non-polling commands to reduce noise
    if !QUIET_COMMANDS.contains(&command) {
        let safe_payload = redact_payload(&payload);
        log::info!("[invoke_command] Command: {}, Payload: {:?}", command, safe_payload);
    }

    let result = match command {
        // Profile commands
        "get_all_profiles" | "get_profile_summaries" | "load_profile" | "save_profile"
        | "delete_profile" | "is_profile_encrypted" | "validate_input"
        | "set_profile_order" | "get_order_index_map" | "ensure_order_indexes" => {
            profiles::handle(state, command, &payload).await
        }

        // Streaming commands
        "start_stream" | "start_all_streams" | "stop_stream" | "stop_all_streams"
        | "retry_stream" | "get_active_stream_count" | "is_group_streaming"
        | "get_active_group_ids" | "toggle_stream_target" | "is_target_disabled" => {
            streaming::handle(state, command, &payload).await
        }

        // FFmpeg / system / log commands
        "get_encoders" | "test_ffmpeg" | "validate_ffmpeg_path" | "test_rtmp_target"
        | "get_recent_logs" | "export_logs" | "download_ffmpeg" | "cancel_ffmpeg_download"
        | "get_bundled_ffmpeg_path" | "check_ffmpeg_update" | "delete_ffmpeg" => {
            ffmpeg::handle(state, command, &payload).await
        }

        // Settings commands
        "get_settings" | "save_settings" | "get_profiles_path" | "export_data"
        | "clear_data" | "rotate_machine_key" => {
            settings::handle(state, command, &payload).await
        }

        // Theme commands
        "list_themes" | "refresh_themes" | "get_theme_tokens" | "install_theme" => {
            themes::handle(state, command, &payload).await
        }

        // OBS commands
        "obs_get_state" | "obs_get_config" | "obs_set_config" | "obs_connect"
        | "obs_disconnect" | "obs_start_stream" | "obs_stop_stream" | "obs_is_connected" => {
            obs::handle(state, command, &payload).await
        }

        // Discord commands
        "discord_test_webhook" | "discord_send_notification" | "discord_reset_cooldown" => {
            discord::handle(state, command, &payload).await
        }

        // Chat commands
        "connect_chat" | "send_chat_message" | "chat_export_log" | "chat_search_session"
        | "disconnect_chat" | "retry_chat_connection" | "disconnect_all_chat"
        | "get_chat_status" | "chat_get_log_status" | "get_platform_chat_status"
        | "is_chat_connected" => {
            chat::handle(state, command, &payload).await
        }

        // OAuth commands
        "oauth_is_configured" | "oauth_start_flow" | "oauth_complete_flow"
        | "oauth_get_account" | "oauth_disconnect" | "oauth_forget"
        | "oauth_refresh_token" | "oauth_get_config" | "oauth_set_config" => {
            oauth::handle(state, command, &payload).await
        }

        _ => Err(format!("Unknown command: {command}")),
    };

    // Only log errors for non-quiet commands, or for unexpected errors
    if let Err(ref e) = result {
        if !QUIET_COMMANDS.contains(&command) || e.contains("Unknown command") {
            log::error!("[invoke_command] Error for {}: {}", command, e);
        }
    }
    result
}
