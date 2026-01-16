use tauri::State;

use crate::models::{ChatConfig, ChatPlatform, ChatPlatformStatus};
use crate::services::ChatManager;

/// Connect to a chat platform
#[tauri::command]
pub async fn connect_chat(
    config: ChatConfig,
    manager: State<'_, ChatManager>,
) -> Result<(), String> {
    manager.connect(config).await
}

/// Disconnect from a specific chat platform
#[tauri::command]
pub async fn disconnect_chat(
    platform: ChatPlatform,
    manager: State<'_, ChatManager>,
) -> Result<(), String> {
    manager.disconnect(platform).await
}

/// Disconnect from all chat platforms
#[tauri::command]
pub async fn disconnect_all_chat(manager: State<'_, ChatManager>) -> Result<(), String> {
    manager.disconnect_all().await
}

/// Get status of all connected chat platforms
#[tauri::command]
pub async fn get_chat_status(
    manager: State<'_, ChatManager>,
) -> Result<Vec<ChatPlatformStatus>, String> {
    Ok(manager.get_status().await)
}

/// Get status of a specific chat platform
#[tauri::command]
pub async fn get_platform_chat_status(
    platform: ChatPlatform,
    manager: State<'_, ChatManager>,
) -> Result<Option<ChatPlatformStatus>, String> {
    Ok(manager.get_platform_status(platform).await)
}

/// Check if any chat platform is connected
#[tauri::command]
pub async fn is_chat_connected(manager: State<'_, ChatManager>) -> Result<bool, String> {
    Ok(manager.is_any_connected().await)
}
