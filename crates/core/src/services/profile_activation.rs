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

use tokio::sync::Mutex;

use crate::errors::CoreError;
use crate::models::{ObsIntegrationDirection, Profile};
use crate::services::profile_manager::ProfileActivatedEvent;
use crate::services::{
    AuthSurveillanceService, ChatManager, EventSink, IntegrationDirection, OAuthService, ObsConfig,
    ObsConnectionStatus, ObsWebSocketHandler, ProfileManager, SettingsManager,
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
    settings: Arc<SettingsManager>,
    oauth: Arc<OAuthService>,
    chat: Arc<ChatManager>,
    obs: Arc<ObsWebSocketHandler>,
    events: Arc<dyn EventSink>,
    surveillance: Arc<AuthSurveillanceService>,
    /// Serialises concurrent `activate()` calls. Two activations of
    /// different profiles racing would let their persisted state
    /// (last_profile, chat config, OBS config) interleave by scheduling
    /// — whoever finished last would win, ignoring which the user
    /// actually requested first. Frontend double-clicks + retry-on-fail
    /// paths can trigger this without an attacker. Held across the
    /// whole cascade (load → OAuth refresh → save → chat → OBS → event).
    activation_lock: Arc<Mutex<()>>,
    /// Test hook: forces the post-refresh persist branch to run even
    /// when no token actually refreshed, so the "activation must re-save
    /// with the original password" regression is testable without a live
    /// OAuth endpoint (provider `token_url`s are `&'static str`).
    #[cfg(test)]
    test_force_refresh_persist: Arc<std::sync::atomic::AtomicBool>,
}

impl ProfileActivationService {
    pub fn new(
        profiles: Arc<ProfileManager>,
        settings: Arc<SettingsManager>,
        oauth: Arc<OAuthService>,
        chat: Arc<ChatManager>,
        obs: Arc<ObsWebSocketHandler>,
        events: Arc<dyn EventSink>,
        surveillance: Arc<AuthSurveillanceService>,
    ) -> Self {
        Self {
            profiles,
            settings,
            oauth,
            chat,
            obs,
            events,
            surveillance,
            activation_lock: Arc::new(Mutex::new(())),
            #[cfg(test)]
            test_force_refresh_persist: Arc::new(std::sync::atomic::AtomicBool::new(false)),
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
        // Serialise: see `activation_lock` field comment for why.
        let _activation_guard = self.activation_lock.lock().await;
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
        let mut must_persist_refresh = !refresh.refreshed.is_empty();
        #[cfg(test)]
        {
            must_persist_refresh |= self
                .test_force_refresh_persist
                .load(std::sync::atomic::Ordering::Relaxed);
        }
        // Anonymous mode must never run salt-less: a legacy or
        // hand-edited profile with `anonymousLogging: true` and no salt
        // used to activate as-is, and every chat username was logged in
        // plaintext until the next save happened to mint one. Generate
        // and persist the salt as part of activation instead.
        if profile.anonymous_logging && profile.anonymous_salt.is_empty() {
            profile.ensure_anonymous_salt();
            must_persist_refresh = true;
        }
        if must_persist_refresh {
            // Re-save WITH the activation password. Passing `None` here
            // used to silently convert a password-encrypted `.mgs`
            // profile to plaintext `.json` (and delete the `.mgs`) the
            // first time activation refreshed a token — the user's
            // deliberate password gate vanished without any error.
            self.profiles
                .save_with_key_encryption(&profile, password)
                .await?;
        }

        // Persist the active profile name to `Settings::last_profile`.
        // Subsequent CLI invocations (which start with empty in-memory
        // state) and HTTP startup's profile auto-load (`lib.rs:2486`)
        // both rely on this to identify the active profile. Prior to
        // this, only obs_websocket + the HTTP startup probe read the
        // field, and nothing wrote it — leaving CLI chat send and other
        // active-profile-aware commands without an active profile.
        match self.settings.load() {
            Ok(mut current) => {
                if current.last_profile.as_deref() != Some(name) {
                    current.last_profile = Some(name.to_string());
                    if let Err(e) = self.settings.save(&current) {
                        log::warn!("Failed to persist last_profile={name}: {e}");
                    }
                }
            }
            Err(e) => log::warn!("Could not load settings to persist last_profile: {e}"),
        }

        self.chat
            .update_profile_chat_settings(profile.settings.chat.clone())
            .await;

        // Push the anonymous-mode policy from core so EVERY transport
        // gets it (the CLI previously never set it — only the HTTP
        // transport's post-activation hook did, so CLI-driven chat
        // sessions logged plaintext usernames with anonymous mode on).
        // Fails the activation outright on an invalid salt: an enabled
        // policy that can't pseudonymise would drop every message.
        self.chat
            .set_anonymous_policy(profile.anonymous_logging, profile.anonymous_salt.clone())?;

        // Same single-source push for the PII policy snapshot the
        // crosspost gate consumes.
        self.chat
            .set_pii_policy(profile.pii_blocklist.clone(), profile.pii_fuzzy);

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
            ObsIntegrationDirection::ObsToSpiritstream => IntegrationDirection::ObsToSpiritstream,
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
    use crate::services::{AuditLogService, OAuthConfig};
    use crate::NoopEventSink;

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
                url: String::new(),
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
        let oauth = Arc::new(OAuthService::new(
            OAuthConfig::default(),
            crate::services::build_secret_store(&data_dir, Some("file")).expect("file store"),
        ));

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
        let oauth = Arc::new(OAuthService::new(
            OAuthConfig::default(),
            crate::services::build_secret_store(&data_dir, Some("file")).expect("file store"),
        ));

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

    /// Build the seven participants directly against a temp dir. Every one is
    /// constructed from local file paths — no keyring, no network, no
    /// subprocess — so the full `activate()` cascade runs in-process.
    fn build_service(data_dir: &std::path::Path) -> ProfileActivationService {
        let profiles = Arc::new(ProfileManager::new(data_dir.to_path_buf()));
        let settings = Arc::new(SettingsManager::new(data_dir.to_path_buf()));
        let oauth = Arc::new(OAuthService::new(
            OAuthConfig::default(),
            crate::services::build_secret_store(data_dir, Some("file")).expect("file store"),
        ));
        let events: Arc<dyn EventSink> = Arc::new(NoopEventSink);
        let chat = Arc::new(ChatManager::new(events.clone(), data_dir.join("logs")));
        let obs = Arc::new(ObsWebSocketHandler::new(data_dir.to_path_buf()));
        let audit = Arc::new(AuditLogService::new_for_tests(data_dir.to_path_buf()).expect("audit log"));
        let surveillance = Arc::new(AuthSurveillanceService::new(audit, events.clone()));
        ProfileActivationService::new(profiles, settings, oauth, chat, obs, events, surveillance)
    }

    /// Happy-path cascade: a healthy (far-future) token and the default OBS
    /// settings (auto_connect=false, no prior session) keep every step on the
    /// in-process path — load, no-op refresh, last_profile persistence, chat
    /// propagation, OBS reconfigure, and consolidated event assembly. Asserts
    /// the structured outcome and the persisted `last_profile`.
    #[tokio::test]
    async fn activate_runs_full_cascade_and_persists_last_profile() {
        let data_dir = fresh_dir();
        std::fs::create_dir_all(data_dir.join("logs")).unwrap();
        let service = build_service(&data_dir);

        let far_future = now_secs() + 3600;
        save_profile_with_oauth(
            &service.profiles,
            "go-live",
            far_future,
            "refresh-token-abc",
        )
        .await;

        let outcome = service.activate("go-live", None).await.expect("activate");

        assert_eq!(outcome.profile.name, "go-live");
        assert_eq!(outcome.event.name, "go-live");
        assert!(outcome.oauth_refresh_failed.is_empty());

        let persisted = service.settings.load().expect("settings load");
        assert_eq!(persisted.last_profile.as_deref(), Some("go-live"));
        let _ = std::fs::remove_dir_all(data_dir);
    }

    /// A second activation of the SAME profile is a no-op for `last_profile`
    /// (the `!=` guard skips the redundant save) and still returns a complete
    /// outcome — proving the cascade is idempotent under the frontend
    /// double-click / retry path the `activation_lock` exists to serialise.
    #[tokio::test]
    async fn activate_is_idempotent_for_repeat_activation() {
        let data_dir = fresh_dir();
        std::fs::create_dir_all(data_dir.join("logs")).unwrap();
        let service = build_service(&data_dir);

        let far_future = now_secs() + 3600;
        save_profile_with_oauth(&service.profiles, "again", far_future, "refresh-token-abc").await;

        service
            .activate("again", None)
            .await
            .expect("first activate");
        let outcome = service
            .activate("again", None)
            .await
            .expect("second activate");

        assert_eq!(outcome.profile.name, "again");
        assert_eq!(
            service.settings.load().unwrap().last_profile.as_deref(),
            Some("again")
        );
        let _ = std::fs::remove_dir_all(data_dir);
    }

    /// Activating a profile that was never saved surfaces the load error from
    /// the very first `?` in the cascade — the orchestrator must not swallow
    /// it into a partial activation.
    #[tokio::test]
    async fn activate_missing_profile_errors() {
        let data_dir = fresh_dir();
        std::fs::create_dir_all(data_dir.join("logs")).unwrap();
        let service = build_service(&data_dir);

        let err = service.activate("does-not-exist", None).await.unwrap_err();
        // Whatever the precise variant, last_profile must not have been touched.
        assert!(service.settings.load().unwrap().last_profile.is_none());
        let _ = err;
        let _ = std::fs::remove_dir_all(data_dir);
    }

    /// Activation must never strip the password envelope. Pre-fix, the
    /// post-OAuth-refresh persist called `save_with_key_encryption(_,
    /// None)`, which writes plaintext `{name}.json` and DELETES
    /// `{name}.mgs` — the first activation that refreshed a token
    /// silently removed the user's password gate. The test-only force
    /// flag drives the persist branch without a live OAuth endpoint.
    #[tokio::test]
    async fn activate_preserves_password_envelope_when_refresh_persists() {
        let data_dir = fresh_dir();
        std::fs::create_dir_all(data_dir.join("logs")).unwrap();
        let service = build_service(&data_dir);
        let password = "correct-horse-battery";

        let profile = Profile {
            id: "locked".into(),
            name: "locked".into(),
            encrypted: true,
            input: RtmpInput {
                input_type: "rtmp".into(),
                bind_address: "127.0.0.1".into(),
                port: 1937,
                application: "live".into(),
                url: String::new(),
            },
            output_groups: vec![],
            settings: ProfileSettings::default(),
            pii_blocklist: vec!["Real Name".into()],
            pii_fuzzy: false,
            anonymous_logging: true,
            anonymous_salt: String::new(),
        };
        service
            .profiles
            .save_with_key_encryption(&profile, Some(password))
            .await
            .expect("save locked fixture");
        let profiles_dir = data_dir.join("profiles");
        assert!(profiles_dir.join("locked.mgs").exists());

        service
            .test_force_refresh_persist
            .store(true, std::sync::atomic::Ordering::Relaxed);
        service
            .activate("locked", Some(password))
            .await
            .expect("activate with password");

        assert!(
            profiles_dir.join("locked.mgs").exists(),
            "activation must keep the password-encrypted .mgs file"
        );
        assert!(
            !profiles_dir.join("locked.json").exists(),
            "activation must not write a plaintext copy of an encrypted profile"
        );

        // Still loadable with the password, secrets intact.
        let reloaded = service
            .profiles
            .load_with_key_decryption("locked", Some(password))
            .await
            .expect("reload after activation");
        assert_eq!(reloaded.pii_blocklist, vec!["Real Name".to_string()]);
        let _ = std::fs::remove_dir_all(data_dir);
    }

    /// A legacy / hand-edited profile with `anonymousLogging: true` and
    /// no salt used to activate as-is and log plaintext usernames until
    /// the next save. Activation must now mint + persist the salt and
    /// push an enabled policy into ChatManager.
    #[tokio::test]
    async fn activate_mints_and_persists_salt_for_legacy_anonymous_profile() {
        let data_dir = fresh_dir();
        std::fs::create_dir_all(data_dir.join("logs")).unwrap();
        let service = build_service(&data_dir);

        // Bypass the manager (whose save path would mint a salt) to
        // simulate the legacy on-disk shape.
        let legacy = Profile {
            id: "legacy".into(),
            name: "legacy".into(),
            encrypted: false,
            input: RtmpInput {
                input_type: "rtmp".into(),
                bind_address: "127.0.0.1".into(),
                port: 1938,
                application: "live".into(),
                url: String::new(),
            },
            output_groups: vec![],
            settings: ProfileSettings::default(),
            pii_blocklist: vec![],
            pii_fuzzy: false,
            anonymous_logging: true,
            anonymous_salt: String::new(),
        };
        std::fs::write(
            data_dir.join("profiles").join("legacy.json"),
            serde_json::to_string_pretty(&legacy).unwrap(),
        )
        .unwrap();

        let outcome = service
            .activate("legacy", None)
            .await
            .expect("legacy activation");
        assert!(
            !outcome.profile.anonymous_salt.is_empty(),
            "activation must mint a salt for an anonymous legacy profile"
        );

        // Persisted, not just in-memory.
        let on_disk = service
            .profiles
            .load_with_key_decryption("legacy", None)
            .await
            .unwrap();
        assert!(!on_disk.anonymous_salt.is_empty());

        // Policy pushed into ChatManager with the minted salt.
        let guard = service.chat.anonymous_policy.read().unwrap();
        let (enabled, salt) = guard.as_ref().expect("policy must be set").clone();
        assert!(enabled);
        assert_eq!(salt, on_disk.anonymous_salt);
        drop(guard);
        let _ = std::fs::remove_dir_all(data_dir);
    }
}
