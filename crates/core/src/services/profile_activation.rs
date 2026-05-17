//! `ProfileActivationService` — orchestrates a single profile activation by
//! composing the participating services. Owning the orchestration in its own
//! service keeps `ProfileManager` focused on persistence + CRUD and keeps
//! `OAuthService` focused on token policy; the transport layer becomes a
//! one-liner that asks for an activation and reacts to the structured
//! `ActivationOutcome` it gets back.
//!
//! Previously this orchestration lived inside `ProfileManager::activate`,
//! which coupled persistence concerns to OAuth refresh + chat propagation +
//! OBS lifecycle. Extracting it eliminates that coupling, matches the
//! rewrite's "thin transport, modular core" rule, and gives both the HTTP
//! and CLI transports a single entry point with identical semantics.

use std::sync::Arc;

use crate::errors::CoreError;
use crate::models::{ObsIntegrationDirection, Profile};
use crate::services::profile_manager::ProfileActivatedEvent;
use crate::services::{
    AuthSurveillanceService, ChatManager, EventSink, IntegrationDirection, OAuthService, ObsConfig,
    ObsConnectionStatus, ObsWebSocketHandler, ProfileManager,
};

/// Refresh leeway for OAuth tokens evaluated during activation. Matches the
/// constant previously used by the HTTP transport's standalone helper — far
/// enough ahead of expiry that the first chat-connect or stream-start after
/// activate doesn't 401.
const OAUTH_REFRESH_LEEWAY_SECS: i64 = 300;

/// Structured result of `ProfileActivationService::activate`. Transports
/// emit `profile.event` on their event bus and translate
/// `oauth_refresh_failed` into transport-shaped follow-up events
/// (HTTP: `oauth_token_expired` over the WebSocket).
#[derive(Debug)]
pub struct ActivationOutcome {
    pub profile: Profile,
    pub event: ProfileActivatedEvent,
    pub oauth_refresh_failed: Vec<String>,
}

/// Composes profile load + OAuth refresh + chat propagation + OBS
/// reconfigure + event payload assembly. Constructed once in
/// `ServiceRegistry` and shared as `Arc<...>` across transports.
#[derive(Clone)]
pub struct ProfileActivationService {
    profiles: Arc<ProfileManager>,
    oauth: Arc<OAuthService>,
    chat: Arc<ChatManager>,
    obs: Arc<ObsWebSocketHandler>,
    events: Arc<dyn EventSink>,
    surveillance: Arc<AuthSurveillanceService>,
}

impl ProfileActivationService {
    pub fn new(
        profiles: Arc<ProfileManager>,
        oauth: Arc<OAuthService>,
        chat: Arc<ChatManager>,
        obs: Arc<ObsWebSocketHandler>,
        events: Arc<dyn EventSink>,
        surveillance: Arc<AuthSurveillanceService>,
    ) -> Self {
        Self {
            profiles,
            oauth,
            chat,
            obs,
            events,
            surveillance,
        }
    }

    /// Run the full activation cascade. Single load, single OAuth-refresh
    /// pass, single chat/OBS propagation, single event-payload assembly.
    /// Encrypted profiles run Argon2id exactly once (m=64 MiB, t=3, p=4) —
    /// previously the transport-side post-load refresh path doubled this.
    pub async fn activate(
        &self,
        name: &str,
        password: Option<&str>,
    ) -> Result<ActivationOutcome, CoreError> {
        let mut profile = self
            .profiles
            .load_with_key_decryption(name, password)
            .await?;

        let refresh = self
            .oauth
            .refresh_profile_tokens(
                &mut profile,
                OAUTH_REFRESH_LEEWAY_SECS,
                Some(&self.surveillance),
            )
            .await?;
        if !refresh.refreshed.is_empty() {
            self.profiles
                .save_with_key_encryption(&profile, None)
                .await?;
        }

        self.chat
            .update_profile_chat_settings(profile.settings.chat.clone())
            .await;

        // Tear down any existing OBS session before reconfigure — otherwise
        // we'd hold a connection authenticated with the prior profile's
        // password while the new config replaces it.
        let prior_status = self.obs.get_state().await.connection_status;
        if matches!(
            prior_status,
            ObsConnectionStatus::Connected | ObsConnectionStatus::Connecting
        ) {
            if let Err(e) = self.obs.disconnect(self.events.clone()).await {
                log::warn!("OBS disconnect during profile activate failed: {e}");
            }
        }

        let obs_settings = &profile.settings.obs;
        let direction = match obs_settings.direction {
            ObsIntegrationDirection::ObsToSpiritstream => {
                IntegrationDirection::ObsToSpiritstream
            }
            ObsIntegrationDirection::SpiritstreamToObs => IntegrationDirection::SpiritstreamToObs,
            ObsIntegrationDirection::Bidirectional => IntegrationDirection::Bidirectional,
            ObsIntegrationDirection::Disabled => IntegrationDirection::Disabled,
        };
        self.obs
            .set_config(ObsConfig {
                host: obs_settings.host.clone(),
                port: obs_settings.port,
                password: obs_settings.password.clone(),
                use_auth: obs_settings.use_auth,
                direction,
                auto_connect: obs_settings.auto_connect,
            })
            .await;

        if obs_settings.auto_connect {
            if let Err(e) = self.obs.connect(self.events.clone()).await {
                log::warn!("OBS auto-connect during profile activate failed: {e}");
            }
        }

        let event = ProfileActivatedEvent::from_profile_public(&profile);
        // Emit the consolidated event from the orchestrator itself so HTTP
        // and CLI see identical bus traffic without re-implementing the
        // emission. Transports may still inspect `outcome.event` for
        // response bodies / CLI JSON output.
        match serde_json::to_value(&event) {
            Ok(value) => self.events.emit("profile_activated", value),
            Err(e) => log::warn!("profile_activated event serialization failed: {e}"),
        }

        Ok(ActivationOutcome {
            profile,
            event,
            oauth_refresh_failed: refresh.failed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{OAuthAccount, OAuthSettings, ProfileSettings, RtmpInput};
    use crate::services::OAuthConfig;

    fn fresh_dir() -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "spiritstream-activation-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(p.join("profiles")).unwrap();
        p
    }

    fn now_secs() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }

    async fn save_profile_with_oauth(
        mgr: &ProfileManager,
        name: &str,
        twitch_expires_at: i64,
        twitch_refresh: &str,
    ) {
        let settings = ProfileSettings {
            oauth: OAuthSettings {
                twitch: OAuthAccount {
                    access_token: "access-token-xyz".into(),
                    refresh_token: twitch_refresh.into(),
                    expires_at: twitch_expires_at,
                    user_id: "twitch-user".into(),
                    username: "twitchhandle".into(),
                    display_name: "Twitch Display".into(),
                },
                ..OAuthSettings::default()
            },
            ..ProfileSettings::default()
        };
        let profile = Profile {
            id: format!("test-{name}"),
            name: name.into(),
            encrypted: false,
            input: RtmpInput {
                input_type: "rtmp".into(),
                bind_address: "127.0.0.1".into(),
                port: 1935,
                application: "live".into(),
            },
            output_groups: vec![],
            settings,
            pii_blocklist: vec![],
            pii_fuzzy: false,
            anonymous_logging: true,
            anonymous_salt: String::new(),
        };
        mgr.save_with_key_encryption(&profile, None)
            .await
            .expect("save plaintext fixture");
    }

    /// A token NOT inside the leeway window must not be touched — refresh is
    /// a no-op for healthy tokens. The expiry field stays intact through the
    /// load/refresh/return loop, and the outcome lists neither refresh nor
    /// failure for the provider.
    #[tokio::test]
    async fn refresh_skips_tokens_outside_leeway() {
        let data_dir = fresh_dir();
        let mgr = ProfileManager::new(data_dir.clone());
        let oauth = Arc::new(OAuthService::new(OAuthConfig::default()));

        let far_future = now_secs() + 3600;
        save_profile_with_oauth(&mgr, "still-fresh", far_future, "refresh-token-abc").await;

        let mut profile = mgr
            .load_with_key_decryption("still-fresh", None)
            .await
            .expect("load profile");
        let outcome = oauth
            .refresh_profile_tokens(&mut profile, 300, None)
            .await
            .expect("refresh must not error on healthy tokens");

        assert!(outcome.refreshed.is_empty());
        assert!(outcome.failed.is_empty());
        assert_eq!(profile.settings.oauth.twitch.expires_at, far_future);
        assert_eq!(
            profile.settings.oauth.twitch.access_token,
            "access-token-xyz"
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }

    /// A profile with NO refresh token + an expiring access token: refresh
    /// helper skips the provider entirely (no `failed` entry — we can't even
    /// attempt without a refresh token). expires_at stays unchanged so the
    /// caller can act on the in-memory profile without re-loading.
    #[tokio::test]
    async fn refresh_skips_provider_without_refresh_token() {
        let data_dir = fresh_dir();
        let mgr = ProfileManager::new(data_dir.clone());
        let oauth = Arc::new(OAuthService::new(OAuthConfig::default()));

        let almost_expired = now_secs() + 60;
        save_profile_with_oauth(&mgr, "no-refresh", almost_expired, "").await;

        let mut profile = mgr
            .load_with_key_decryption("no-refresh", None)
            .await
            .expect("load profile");
        let outcome = oauth
            .refresh_profile_tokens(&mut profile, 300, None)
            .await
            .expect("missing refresh_token must not abort the loop");

        assert!(outcome.refreshed.is_empty());
        assert!(outcome.failed.is_empty());
        assert_eq!(profile.settings.oauth.twitch.expires_at, almost_expired);
        let _ = std::fs::remove_dir_all(data_dir);
    }
}
