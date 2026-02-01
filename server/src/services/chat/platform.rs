use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::models::{ChatMessage, ChatCredentials, ChatConnectionStatus};

/// Result type for platform operations
pub type PlatformResult<T> = Result<T, PlatformError>;

/// Errors that can occur during platform operations
#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Authentication error: {0}")]
    #[allow(dead_code)]
    Authentication(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Network error: {0}")]
    #[allow(dead_code)]
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
    /// Connect to the platform and start receiving messages
    async fn connect(
        &mut self,
        credentials: ChatCredentials,
        message_tx: mpsc::UnboundedSender<ChatMessage>,
    ) -> PlatformResult<()>;

    /// Disconnect from the platform
    async fn disconnect(&mut self) -> PlatformResult<()>;

    /// Get the current connection status
    fn status(&self) -> ChatConnectionStatus;

    /// Get the number of messages received
    fn message_count(&self) -> u64;

    /// Get the platform name
    #[allow(dead_code)]
    fn platform_name(&self) -> &'static str;

    /// Check if currently connected
    fn is_connected(&self) -> bool {
        matches!(self.status(), ChatConnectionStatus::Connected)
    }
}

/// Type alias for a boxed chat platform
#[allow(dead_code)]
pub type BoxedPlatform = Box<dyn ChatPlatform>;

/// Shared chat platform reference
#[allow(dead_code)]
pub type SharedPlatform = Arc<tokio::sync::Mutex<BoxedPlatform>>;
