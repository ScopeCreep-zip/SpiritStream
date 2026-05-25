use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// OBS connection status
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../../../packages/types/src/generated/")]
pub enum ObsConnectionStatus {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Error,
}

/// OBS streaming status
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../../../packages/types/src/generated/")]
pub enum ObsStreamStatus {
    Inactive,
    Starting,
    Active,
    Stopping,
    #[default]
    Unknown,
}

/// Integration directionality — controls how stream state syncs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../../../../packages/types/src/generated/")]
pub enum IntegrationDirection {
    /// OBS controls SpiritStream (OBS start → SpiritStream start)
    ObsToSpiritstream,
    /// SpiritStream controls OBS (SpiritStream start → OBS start)
    SpiritstreamToObs,
    /// Bidirectional sync (either can trigger the other)
    Bidirectional,
    /// No automatic sync
    #[default]
    Disabled,
}

/// OBS WebSocket configuration (stored in settings)
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../../packages/types/src/generated/")]
pub struct ObsConfig {
    /// WebSocket host (e.g., "localhost")
    pub host: String,
    /// WebSocket port (default: 4455 for OBS 28+)
    pub port: u16,
    /// Authentication password (encrypted at rest)
    pub password: String,
    /// Whether to use authentication
    pub use_auth: bool,
    /// Integration direction
    pub direction: IntegrationDirection,
    /// Auto-connect on startup
    pub auto_connect: bool,
}

impl ObsConfig {
    pub fn default_config() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 4455,
            password: String::new(),
            use_auth: false,
            direction: IntegrationDirection::Disabled,
            auto_connect: false,
        }
    }
}

/// Current OBS state snapshot
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../../packages/types/src/generated/")]
pub struct ObsState {
    pub connection_status: ObsConnectionStatus,
    pub stream_status: ObsStreamStatus,
    pub error_message: Option<String>,
    pub obs_version: Option<String>,
    pub websocket_version: Option<String>,
}
