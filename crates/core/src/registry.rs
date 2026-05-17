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
    AuditLogService, AuthSurveillanceService, ChatManager, DiscordWebhookService, EventSink,
    FFmpegHandler, FFmpegLocator, OAuthConfig, OAuthService, ObsWebSocketHandler,
    ProfileActivationService, ProfileManager, SafetyService, SettingsManager, ThemeManager,
};
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
    /// Mirrors the `SafetyService` shape — Arc<participants> in,
    /// single verb method out.
    pub profile_activation: Arc<ProfileActivationService>,
    pub events: Arc<dyn EventSink>,
    pub data_dir: PathBuf,
    pub log_dir: PathBuf,
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

        let profiles = Arc::new(ProfileManager::new(opts.data_dir.clone()));
        let settings = Arc::new(SettingsManager::new(opts.data_dir.clone()));
        let themes = Arc::new(ThemeManager::new(
            opts.data_dir.clone(),
            opts.themes_dir.clone(),
        ));
        let ffmpeg = Arc::new(FFmpegHandler::new_with_custom_path(
            opts.data_dir.clone(),
            opts.custom_ffmpeg_path,
        ));
        let ffmpeg_locator = Arc::new(FFmpegLocator::new());
        let obs = Arc::new(ObsWebSocketHandler::new(opts.data_dir.clone()));
        let discord = Arc::new(DiscordWebhookService::new());
        let chat = Arc::new(ChatManager::new(opts.events.clone(), opts.log_dir.clone()));
        let oauth = Arc::new(OAuthService::new(OAuthConfig::default()));
        let audit = Arc::new(AuditLogService::new(opts.data_dir.clone())?);
        let safety = Arc::new(SafetyService::new(
            ffmpeg.clone(),
            chat.clone(),
            obs.clone(),
            audit.clone(),
            opts.events.clone(),
            None, // SecretStore wiring lands when callers swap to it.
        ));
        let auth_surveillance = Arc::new(AuthSurveillanceService::new(
            audit.clone(),
            opts.events.clone(),
        ));

        // Profile-activation orchestrator. Constructed after every
        // participant so each Arc is cloned exactly once into the service.
        let profile_activation = Arc::new(ProfileActivationService::new(
            profiles.clone(),
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
            events: opts.events,
            data_dir: opts.data_dir,
            log_dir: opts.log_dir,
        })
    }
}

/// A `EventSink` that does nothing. Useful for one-shot CLI commands that
/// invoke a service method without subscribing to events.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopEventSink;

impl EventSink for NoopEventSink {
    fn emit(&self, _event: &str, _payload: serde_json::Value) {}
}
