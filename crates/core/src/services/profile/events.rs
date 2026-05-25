//! Event types emitted by `ProfileManager::activate()` and consumed by
//! transports as the canonical "a profile is now active" payload.
//!
//! Plan-driven `ProfileActivated` payload — the consolidated state the
//! transport emits whenever a profile is activated. Replaces the frontend
//! `applyProfileSettings` cascade that used to assemble this from individual
//! `Profile.settings` fields. Every UI store listens for one event instead
//! of being driven by a synchronous procedural cascade.

use crate::models::Profile;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../../packages/types/src/generated/")]
pub struct ProfileActivatedEvent {
    pub name: String,
    pub theme_id: String,
    pub language: String,
    pub show_notifications: bool,
    pub encrypt_stream_keys: bool,
    pub obs: ActivatedObs,
    pub chat: crate::models::ChatSettings,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../../packages/types/src/generated/")]
pub struct ActivatedObs {
    pub host: String,
    pub port: u16,
    pub use_auth: bool,
    pub direction: crate::models::ObsIntegrationDirection,
    pub auto_connect: bool,
}

impl ProfileActivatedEvent {
    /// Public helper for transports that need to construct the event shape
    /// without re-running the full `activate()` orchestration — e.g. when
    /// re-emitting after a save to an already-active profile.
    pub fn from_profile_public(profile: &Profile) -> Self {
        Self::from_profile(profile)
    }

    fn from_profile(profile: &Profile) -> Self {
        let s = &profile.settings;
        Self {
            name: profile.name.clone(),
            theme_id: s.theme_id.clone(),
            language: s.language.clone(),
            show_notifications: s.show_notifications,
            encrypt_stream_keys: s.encrypt_stream_keys,
            obs: ActivatedObs {
                host: s.obs.host.clone(),
                port: s.obs.port,
                use_auth: s.obs.use_auth,
                direction: s.obs.direction,
                auto_connect: s.obs.auto_connect,
            },
            chat: s.chat.clone(),
        }
    }
}
