//! `ServiceRegistry` — the single, canonical place that constructs every
//! core service. Every transport adapter (`transport-http`, `transport-cli`,
//! future `transport-veilid`, and the Tauri 2 mobile shell) builds the same
//! registry and hands it to its own handler layer.
//!
//! This is intentionally narrow: it owns service *construction*, not service
//! *behavior*. Transports may decorate the registry with their own state
//! (HTTP rate limiter, CLI output formatter, etc.), but the registry itself
//! has no transport types.

use std::path::PathBuf;
use std::sync::Arc;

use crate::services::{
    AuditLogService, AuthSurveillanceService, ChatManager, ConfirmTokenService,
    DiscordWebhookService, EventSink, FFmpegHandler, FFmpegLocator, OAuthConfig, OAuthService,
    ObsWebSocketHandler, ProfileActivationService, ProfileManager, SafetyService, SettingsManager,
    ThemeManager,
};
use crate::traits::SecretStore;
use crate::CoreError;

/// Inputs required to build a `ServiceRegistry`.
///
/// Every input is explicit — services never read environment variables or
/// resolve OS paths on their own. Transports decide what to feed in.
pub struct ServiceRegistryOptions {
    /// Top-level data directory for profiles, settings, machine key, and per-profile state.
    pub data_dir: PathBuf,
    /// Themes directory (the bundled `themes/` copy on disk).
    pub themes_dir: PathBuf,
    /// Per-service log directory. Owned by `ChatManager` for chat-session logs.
    pub log_dir: PathBuf,
    /// Optional override for the FFmpeg binary path. `None` means autodetect.
    pub custom_ffmpeg_path: Option<String>,
    /// Sink for service events (stream stats, chat messages, theme changes, …).
    /// HTTP transport supplies its broadcast bus; CLI supplies a stdout writer
    /// for `spiritstream-cli events watch`, or a no-op sink for one-shot commands.
    pub events: Arc<dyn EventSink>,
    /// Single secret store impl chosen at startup via `build_secret_store`.
    /// Held by `SafetyService` so panic-disconnect can purge in-memory
    /// secret caches; passed in here so callers (transport-http, CLI,
    /// Tauri mobile shell) all funnel through one selection point.
    pub secret_store: Arc<dyn SecretStore>,
}

/// Fully-constructed service registry.
///
/// All fields are `Arc<…>` so transports can clone the handles freely. The
/// registry itself is cheap to clone for the same reason.
#[derive(Clone)]
pub struct ServiceRegistry {
    pub profiles: Arc<ProfileManager>,
    pub settings: Arc<SettingsManager>,
    pub themes: Arc<ThemeManager>,
    pub ffmpeg: Arc<FFmpegHandler>,
    /// Discovery + upstream version-check helper. Read-only — the
    /// runtime download / install flow was retired in favour of
    /// build-time bundling (macOS / Windows Tauri sidecar) and distro
    /// packaging (Linux .deb / .rpm).
    pub ffmpeg_locator: Arc<FFmpegLocator>,
    pub obs: Arc<ObsWebSocketHandler>,
    pub discord: Arc<DiscordWebhookService>,
    pub chat: Arc<ChatManager>,
    pub oauth: Arc<OAuthService>,
    /// Append-only audit log used by safety/panic, PII filter,
    /// OAuth refresh hooks, and the audit-log UI.
    pub audit: Arc<AuditLogService>,
    /// Coordinates the panic-disconnect flow.
    pub safety: Arc<SafetyService>,
    /// Tracks OAuth refresh events for anomaly detection.
    pub auth_surveillance: Arc<AuthSurveillanceService>,
    /// Orchestrates profile activation: composes `profiles`, `oauth`,
    /// `chat`, `obs`, and `auth_surveillance` so transports stay thin.
    /// Mirrors the `SafetyService` shape — `Arc<participants>` in,
    /// single verb method out.
    pub profile_activation: Arc<ProfileActivationService>,
    /// One-shot confirmation tokens for destructive operations
    /// (clear-data, machine-key rotate, revoke-all-sessions). The HTTP
    /// transport gates the destructive endpoints behind a
    /// `X-Confirm-Token`; the CLI exposes `confirm-token issue --intent`
    /// for scripted use. Same service instance across transports so a
    /// token issued via one is consumable via the other (Q6).
    pub confirm_tokens: Arc<ConfirmTokenService>,
    /// Cross-process HTTP session set (hashed, file-backed under
    /// `run/sessions.json`). The HTTP transport validates cookies
    /// against it; the CLI lists/revokes through it.
    pub sessions: Arc<crate::services::SessionStore>,
    pub events: Arc<dyn EventSink>,
    pub data_dir: PathBuf,
    pub log_dir: PathBuf,
    pub themes_dir: PathBuf,
}

impl ServiceRegistry {
    /// Construct every service. Idempotent — each call produces a fresh
    /// independent registry, suitable for an isolated test data dir.
    ///
    /// This intentionally does *not* perform side effects beyond filesystem
    /// directory creation: no theme sync, no FFmpeg version check, no
    /// background tasks. Each transport runs its own startup orchestration
    /// against the returned registry.
    pub fn build(opts: ServiceRegistryOptions) -> Result<Self, CoreError> {
        std::fs::create_dir_all(&opts.data_dir).map_err(|e| CoreError::Internal {
            context: format!("data_dir create: {e}"),
        })?;
        std::fs::create_dir_all(&opts.log_dir).map_err(|e| CoreError::Internal {
            context: format!("log_dir create: {e}"),
        })?;

        // Repair an interrupted machine-key rotation BEFORE anything
        // derives keys (AuditLogService's HMAC key, profile load, secret
        // store). Without this, a crash mid-rotation either bricked the
        // install (key gone) or silently minted a fresh key and turned
        // every stored secret into garbage. The audit entry for the
        // recovery is recorded right after `audit` is constructed below
        // (the audit service itself needs the recovered key).
        let rotation_recovery =
            crate::services::Encryption::recover_interrupted_rotation(&opts.data_dir)?;
        match rotation_recovery {
            crate::services::RotationRecovery::Clean => {}
            crate::services::RotationRecovery::RolledBack => {
                log::warn!(
                    "Recovered from interrupted key rotation by rolling back to the previous \
                     key and restoring profiles from backup — re-run rotation when ready"
                );
            }
            crate::services::RotationRecovery::Promoted => {
                log::warn!(
                    "Recovered from interrupted key rotation by promoting the pending key — \
                     the rotation is now complete"
                );
            }
        }

        let profiles = Arc::new(ProfileManager::new(opts.data_dir.clone()));
        let settings = Arc::new(SettingsManager::new(opts.data_dir.clone()));
        let themes = Arc::new(ThemeManager::new(
            opts.data_dir.clone(),
            opts.themes_dir.clone(),
        ));
        let ffmpeg = Arc::new(FFmpegHandler::new_with_custom_path(
            opts.data_dir.clone(),
            opts.custom_ffmpeg_path,
        )?);
        let ffmpeg_locator = Arc::new(FFmpegLocator::new()?);
        let obs = Arc::new(ObsWebSocketHandler::new(opts.data_dir.clone()));
        let discord = Arc::new(DiscordWebhookService::new(opts.data_dir.clone()));
        let chat = Arc::new(ChatManager::new(opts.events.clone(), opts.log_dir.clone()));
        let oauth = Arc::new(OAuthService::new(
            OAuthConfig::default(),
            opts.secret_store.clone(),
        ));
        // Keyed phrase-id secret for the PII filter's audit identifiers
        // (HMAC, not bare SHA-256 — blocklist phrases are guessable, so
        // an unkeyed hash would let anyone holding the audit log confirm
        // candidate names by hashing them).
        let phrase_id_key = crate::services::Encryption::derive_machine_subkey(
            &opts.data_dir,
            crate::services::PHRASE_ID_KEY_INFO,
        )?;
        let audit = Arc::new(AuditLogService::new(
            opts.data_dir.clone(),
            opts.secret_store.clone(),
        )?);
        // Now that the chain is writable, record the startup rotation
        // recovery (if any) so the user has a durable record of what
        // happened to their keys.
        let recovery_action = match rotation_recovery {
            crate::services::RotationRecovery::Clean => None,
            crate::services::RotationRecovery::RolledBack => {
                Some(crate::services::AuditAction::KeyRotationRolledBack)
            }
            crate::services::RotationRecovery::Promoted => {
                Some(crate::services::AuditAction::KeyRotationRecovered)
            }
        };
        if let Some(action) = recovery_action {
            if let Err(e) = audit.record(action) {
                log::error!("failed to append key-rotation recovery audit entry: {e}");
            }
        }
        let safety = Arc::new(SafetyService::new(
            ffmpeg.clone(),
            chat.clone(),
            obs.clone(),
            audit.clone(),
            opts.events.clone(),
            opts.secret_store.clone(),
            phrase_id_key,
        ));
        // Crosspost PII gate: ChatManager refuses to re-broadcast
        // inbound text until this guard is wired (fail loud, never
        // crosspost unchecked).
        chat.set_outbound_guard(safety.clone());
        let auth_surveillance = Arc::new(AuthSurveillanceService::new(
            audit.clone(),
            opts.events.clone(),
        ));
        let confirm_tokens = Arc::new(ConfirmTokenService::new(&opts.data_dir));
        // Cross-process HTTP session set (file-backed, hashed). Shared so
        // the CLI's `session revoke-all` genuinely revokes a running
        // server's sessions.
        let sessions = Arc::new(crate::services::SessionStore::new(&opts.data_dir));

        // Profile-activation orchestrator. Constructed after every
        // participant so each Arc is cloned exactly once into the service.
        let profile_activation = Arc::new(ProfileActivationService::new(
            profiles.clone(),
            settings.clone(),
            oauth.clone(),
            chat.clone(),
            obs.clone(),
            opts.events.clone(),
            auth_surveillance.clone(),
        ));

        // Wire the OBS↔SpiritStream cascade. Both services hold each
        // other via setters (set once, held for process lifetime) so
        // there's no cyclic build-time dep. The frontend stays a thin
        // wrapper: it never decides "should we trigger?" — core does.
        obs.set_cascade_deps(crate::services::ObsCascadeDeps {
            profiles: profiles.clone(),
            settings: settings.clone(),
            ffmpeg: ffmpeg.clone(),
        });
        let trigger: Arc<dyn crate::services::ObsTrigger> = obs.clone();
        ffmpeg.set_obs_trigger(trigger);

        // Wire audit-log into ThemeManager post-construction so theme
        // validation failures generate `ThemeValidationFailed` audit
        // entries. Pre-wiring there is no cyclic dep, just a clean
        // ordering: AuditLogService is constructed above before this
        // call. The frontend reads audit entries via `audit log` CLI
        // or `GET /api/v1/audit/log` — operators get a queryable
        // trace of accessibility / theme regressions.
        themes.set_audit_log(audit.clone());

        // Same post-construction pattern for ChatManager so chat
        // mutations (`ChatMessageSent`, `ChatPlatformConnected`,
        // `ChatPlatformDisconnected`) reach the HMAC chain. Without
        // this wiring chat operations still succeed — they just don't
        // record. Matches the ThemeManager degraded-mode behavior.
        chat.set_audit_log(audit.clone());

        // G2: same wiring for ProfileManager. Pre-G2 the
        // `ProfileSaved` and `ProfileDeleted` variants existed but had
        // nowhere to fire from — the manager lacked a handle to the
        // audit chain.
        profiles.set_audit_log(audit.clone());

        // H9: same wiring for DiscordWebhookService so go-live
        // notification posts (and cooldown-skipped attempts) reach
        // the HMAC chain.
        discord.set_audit_log(audit.clone());

        Ok(ServiceRegistry {
            profiles,
            settings,
            themes,
            ffmpeg,
            ffmpeg_locator,
            obs,
            discord,
            chat,
            oauth,
            audit,
            safety,
            auth_surveillance,
            profile_activation,
            confirm_tokens,
            sessions,
            events: opts.events,
            data_dir: opts.data_dir,
            log_dir: opts.log_dir,
            themes_dir: opts.themes_dir,
        })
    }

    /// Rotate the machine key with the cross-transport preconditions and
    /// bookkeeping applied. Both the HTTP handler and the CLI command
    /// MUST route through here (not `Encryption::rotate_machine_key`
    /// directly) so the rules can't drift between transports:
    ///
    /// 1. Refuses while any stream is live — rotation rewrites every
    ///    profile while FFmpeg may be reading them, and a mid-stream
    ///    failure would force a rollback during a broadcast.
    /// 2. Records `MachineKeyRotated` in the audit chain.
    pub fn rotate_machine_key_checked(
        &self,
        unlocked_passwords: &std::collections::HashMap<String, String>,
    ) -> Result<crate::services::RotationReport, CoreError> {
        if self.ffmpeg.active_count() > 0 {
            return Err(CoreError::ValidationFailed {
                reasons: vec![crate::errors::ValidationIssue {
                    code: "rotation_while_streaming".into(),
                    message: "Machine-key rotation is not allowed while streams are live. \
                              Stop all streams first."
                        .into(),
                    path: None,
                }],
            });
        }
        let profiles_dir = self.data_dir.join("profiles");
        let report = crate::services::Encryption::rotate_machine_key(
            &self.data_dir,
            &profiles_dir,
            unlocked_passwords,
        )?;
        if let Err(e) = self
            .audit
            .record(crate::services::AuditAction::MachineKeyRotated {
                profiles_updated: report.profiles_updated,
                keys_reencrypted: report.keys_reencrypted,
            })
        {
            log::error!("failed to append MachineKeyRotated audit entry: {e}");
        }
        Ok(report)
    }
}

/// A `EventSink` that does nothing. Useful for one-shot CLI commands that
/// invoke a service method without subscribing to events.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopEventSink;

impl EventSink for NoopEventSink {
    fn emit(&self, _event: &str, _payload: serde_json::Value) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::build_secret_store;

    fn options(root: &std::path::Path) -> ServiceRegistryOptions {
        ServiceRegistryOptions {
            data_dir: root.join("data"),
            themes_dir: root.join("themes"),
            log_dir: root.join("logs"),
            custom_ffmpeg_path: None,
            events: Arc::new(NoopEventSink),
            // File-backed store so the test never touches the OS keyring.
            secret_store: build_secret_store(&root.join("data"), Some("file")).unwrap(),
        }
    }

    #[tokio::test]
    async fn build_creates_data_and_log_dirs() {
        let dir = tempfile::tempdir().expect("tempdir");
        let opts = options(dir.path());
        let data_dir = opts.data_dir.clone();
        let log_dir = opts.log_dir.clone();

        let registry = ServiceRegistry::build(opts).expect("build registry");

        assert!(data_dir.is_dir(), "data_dir created");
        assert!(log_dir.is_dir(), "log_dir created");
        assert_eq!(registry.data_dir, data_dir);
        assert_eq!(registry.log_dir, log_dir);
    }

    #[tokio::test]
    async fn build_is_idempotent_across_calls() {
        let dir = tempfile::tempdir().expect("tempdir");
        // Two independent registries against the same dirs must both build.
        let first = ServiceRegistry::build(options(dir.path())).expect("first build");
        let second = ServiceRegistry::build(options(dir.path())).expect("second build");
        assert_eq!(first.data_dir, second.data_dir);
        assert_eq!(first.log_dir, second.log_dir);
    }

    #[tokio::test]
    async fn build_clone_shares_service_handles() {
        let dir = tempfile::tempdir().expect("tempdir");
        let registry = ServiceRegistry::build(options(dir.path())).expect("build registry");
        let cloned = registry.clone();
        // Cloning the registry must share the same underlying Arc allocations,
        // not deep-copy the services.
        assert!(Arc::ptr_eq(&registry.profiles, &cloned.profiles));
        assert!(Arc::ptr_eq(&registry.audit, &cloned.audit));
        assert!(Arc::ptr_eq(&registry.chat, &cloned.chat));
        assert!(Arc::ptr_eq(&registry.safety, &cloned.safety));
        assert!(Arc::ptr_eq(
            &registry.profile_activation,
            &cloned.profile_activation
        ));
    }
}
