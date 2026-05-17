//! Authentication / authorization abstraction.
//!
//! Today the only credential is a bearer token (`SPIRITSTREAM_API_TOKEN`).
//! Future transports introduce additional credential types — OAuth subject,
//! Veilid keypair, platform-issued attestations.

use serde::{Deserialize, Serialize};

use crate::CoreError;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Credential {
    BearerToken { value: String },
    OAuthAccessToken { provider: String, value: String },
    VeilidKeypair { public_key: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    /// Stable opaque identifier within the running server.
    pub subject: String,
    /// Optional human-friendly label for logs and UI.
    pub label: Option<String>,
}

pub trait IdentityProvider: Send + Sync {
    fn authenticate(&self, credential: &Credential) -> Result<Identity, CoreError>;
}
