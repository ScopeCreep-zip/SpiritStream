use async_trait::async_trait;
use log::error;
use tokio::sync::mpsc;

use crate::models::{ChatConnectionStatus, ChatCredentials, ChatMessage};

use super::platform::{ChatPlatform, PlatformError, PlatformResult};

/// TikTok Live chat connector.
///
/// TikTok Live uses a proprietary protobuf-over-WebSocket protocol with
/// room-ID lookup and signed auth params. SpiritStream's Rust core does
/// not yet implement that protocol. A working integration needs:
///
/// 1. Fetch room_id from `https://www.tiktok.com/@{username}/live`
/// 2. Build the signed `wss://webcast.tiktok.com/webcast/im/fetch/` URL
/// 3. Parse `WebcastChatMessage` / `WebcastMemberMessage` protobuf
///    frames (port `.proto` defs from the TikTok-Live-Connector
///    project; consume with `prost`)
/// 4. Pong replies + cursor-based pagination for chat history
///
/// Until that lands, `connect()` reports the platform as unsupported so
/// the frontend can surface a clear message instead of failing under
/// a misleading "connection error". Tracking note lives in the chat
/// blockers doc next to the rewrite plan.
pub struct TikTokConnector {
    status: ChatConnectionStatus,
    message_count: u64,
}

impl TikTokConnector {
    pub fn new() -> Self {
        Self {
            status: ChatConnectionStatus::Disconnected,
            message_count: 0,
        }
    }
}

#[async_trait]
impl ChatPlatform for TikTokConnector {
    async fn connect(
        &mut self,
        _credentials: ChatCredentials,
        _message_tx: mpsc::UnboundedSender<ChatMessage>,
    ) -> PlatformResult<()> {
        error!("TikTok Live chat is not yet implemented; refusing to connect");
        self.status = ChatConnectionStatus::Error;
        Err(PlatformError::Platform(
            "TikTok Live chat is not yet implemented in SpiritStream".to_string(),
        ))
    }

    async fn disconnect(&mut self) -> PlatformResult<()> {
        Err(PlatformError::NotConnected)
    }

    fn status(&self) -> ChatConnectionStatus {
        self.status
    }

    fn message_count(&self) -> u64 {
        self.message_count
    }

    fn platform_name(&self) -> &'static str {
        "tiktok"
    }
}

impl Default for TikTokConnector {
    fn default() -> Self {
        Self::new()
    }
}
