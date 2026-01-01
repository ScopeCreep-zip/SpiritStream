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

    /// Optional theme customization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<Theme>,
}

/// Optional theme customization for a profile
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    pub primary_color: Option<String>,
    pub accent_color: Option<String>,
}

impl Profile {
    /// Create a new empty profile with the given name
    pub fn new(name: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            incoming_url: String::new(),
            output_groups: Vec::new(),
            theme: None,
        }
    }
}
