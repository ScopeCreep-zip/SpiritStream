use async_trait::async_trait;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::mpsc;

use crate::models::{ChatConnectionStatus, ChatCredentials, ChatMessage};

use super::platform::{ChatPlatform, PlatformError, PlatformResult};

/// Stripchat connector placeholder.
///
/// Public Stripchat developer docs currently expose studio stats APIs,
/// but not a stable public chat ingest/send API suitable for a unified
/// external chat connector. Keep this connector explicit so UI/status can
/// surface a clear reason.
pub struct StripchatConnector {
    status: Arc<AtomicU8>,
    message_count: Arc<AtomicU64>,
    last_error: Arc<StdMutex<Option<String>>>,
}

impl StripchatConnector {
    pub fn new() -> Self {
        Self {
            status: Arc::new(AtomicU8::new(ChatConnectionStatus::Disconnected.to_u8())),
            message_count: Arc::new(AtomicU64::new(0)),
            last_error: Arc::new(StdMutex::new(None)),
        }
    }
}

#[async_trait]
impl ChatPlatform for StripchatConnector {
    async fn connect(
        &mut self,
        credentials: ChatCredentials,
        _message_tx: mpsc::UnboundedSender<ChatMessage>,
    ) -> PlatformResult<()> {
        let username = match credentials {
            ChatCredentials::Stripchat { username } => username,
            _ => {
                self.status
                    .store(ChatConnectionStatus::Error.to_u8(), Ordering::Relaxed);
                if let Ok(mut guard) = self.last_error.lock() {
                    *guard = Some("Expected Stripchat credentials".to_string());
                }
                return Err(PlatformError::InvalidConfig(
                    "Expected Stripchat credentials".to_string(),
                ));
            }
        };

        self.status
            .store(ChatConnectionStatus::Connecting.to_u8(), Ordering::Relaxed);

        let message = format!(
            "Stripchat chat integration is not available yet for username '{username}'. \
Public docs currently expose studio stats APIs, not a stable chat API."
        );

        self.status
            .store(ChatConnectionStatus::Error.to_u8(), Ordering::Relaxed);
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = Some(message.clone());
        }

        Err(PlatformError::Platform(message))
    }

    async fn disconnect(&mut self) -> PlatformResult<()> {
        self.status
            .store(ChatConnectionStatus::Disconnected.to_u8(), Ordering::Relaxed);
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = None;
        }
        Ok(())
    }

    fn status(&self) -> ChatConnectionStatus {
        ChatConnectionStatus::from_u8(self.status.load(Ordering::Relaxed))
    }

    fn message_count(&self) -> u64 {
        self.message_count.load(Ordering::Relaxed)
    }

    fn platform_name(&self) -> &'static str {
        "stripchat"
    }

    fn can_send(&self) -> bool {
        false
    }

    fn last_error(&self) -> Option<String> {
        self.last_error.lock().ok().and_then(|e| e.clone())
    }
}

impl Default for StripchatConnector {
    fn default() -> Self {
        Self::new()
    }
}
