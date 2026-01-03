// Profile Model
// Top-level configuration entity

use serde::{Deserialize, Serialize};
use crate::models::OutputGroup;

/// A streaming profile containing all configuration for a stream setup
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// Unique identifier
    pub id: String,

    /// User-friendly name
    pub name: String,

    /// RTMP source URL
    pub incoming_url: String,

    /// Encoding configurations with their targets
    pub output_groups: Vec<OutputGroup>,
}

