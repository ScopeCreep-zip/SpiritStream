use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::models::{ChatConnectionStatus, ChatCredentials, ChatMessage};

/// Result type for platform operations
pub type PlatformResult<T> = Result<T, PlatformError>;

/// Errors that can occur during platform operations
#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Authentication error: {0}")]
    Authentication(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Already connected")]
    AlreadyConnected,

    #[error("Not connected")]
    NotConnected,

    #[error("Platform error: {0}")]
    Platform(String),
}

/// Trait that all chat platform connectors must implement
#[async_trait]
pub trait ChatPlatform: Send + Sync {
    /// Connect to the platform and start receiving messages. The
    /// `message_tx` is a BOUNDED channel — connectors should call
    /// `try_send` (non-blocking) rather than `send().await` to avoid
    /// back-pressuring the upstream platform's read loop when the
    /// chat-manager's consumer falls behind. On `TrySendError::Full`,
    /// log + drop the message (losing 1 chat message during a flood is
    /// preferable to disconnecting from the platform). On
    /// `TrySendError::Closed`, the manager is shutting down — exit
    /// the connector loop cleanly.
    async fn connect(
        &mut self,
        credentials: ChatCredentials,
        message_tx: mpsc::Sender<ChatMessage>,
    ) -> PlatformResult<()>;

    /// Disconnect from the platform
    async fn disconnect(&mut self) -> PlatformResult<()>;

    /// Get the current connection status
    fn status(&self) -> ChatConnectionStatus;

    /// Get the number of messages received
    fn message_count(&self) -> u64;

    /// Identifier the connector reports for itself. Must match the
    /// `ChatPlatform` enum variant the factory was asked to build —
    /// `ChatManager` debug-asserts this to catch factory bugs where a
    /// connector is registered under the wrong enum.
    fn platform_name(&self) -> &'static str;

    /// Check if currently connected
    fn is_connected(&self) -> bool {
        matches!(self.status(), ChatConnectionStatus::Connected)
    }

    /// Send a chat message (if supported by the platform and authenticated)
    async fn send_message(&mut self, _message: String) -> PlatformResult<()> {
        Err(PlatformError::Platform(
            "Sending messages is not supported for this platform".to_string(),
        ))
    }

    /// Whether the current connection can send messages
    fn can_send(&self) -> bool {
        false
    }

    /// Get last error message (if any)
    fn last_error(&self) -> Option<String> {
        None
    }

    /// Update the OAuth access token for platforms that poll APIs.
    /// Default is a no-op. YouTube overrides this to swap the token mid-session.
    fn update_token(&mut self, _token: String) {}
}

/// Type alias for a boxed chat platform — used by `ChatManager`'s
/// `HashMap<ChatPlatform, BoxedPlatform>` registry.
pub type BoxedPlatform = Box<dyn ChatPlatform>;
