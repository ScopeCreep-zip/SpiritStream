// FFmpegHandler Service
// Manages FFmpeg processes for streaming with real-time stats

use crate::errors::{CoreError, ValidationIssue};
use crate::models::{OutputGroup, Profile, StreamStats};
use crate::services::PlatformRegistry;
use crate::services::{emit_event, EventSink};
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::net::UdpSocket;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU16, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Encoding-config bounds the plan calls out for `StreamService::validate_config`.
/// Out-of-range values surface as `CoreError::InvalidStreamConfig` so the
/// frontend can highlight specific fields rather than parsing message strings.
pub const VIDEO_BITRATE_MIN_KBPS: u32 = 500;
pub const VIDEO_BITRATE_MAX_KBPS: u32 = 50_000;
pub const KEYFRAME_INTERVAL_MIN_SECS: u32 = 1;
pub const KEYFRAME_INTERVAL_MAX_SECS: u32 = 10;
pub const FPS_MIN: u32 = 1;
pub const FPS_MAX: u32 = 240;

fn lock_poisoned<E: std::fmt::Display>(e: E) -> CoreError {
    CoreError::Internal {
        context: format!("Lock poisoned: {e}"),
    }
}

fn ffmpeg_internal<S: Into<String>>(msg: S) -> CoreError {
    CoreError::Internal {
        context: msg.into(),
    }
}

/// Parse a bitrate string ("6000k", "8M", "8000000") into kilobits per
/// second. Returns `None` for unparseable input — callers map that to a
/// `ValidationIssue`. Matches the frontend `parseBitrateToKbps` helper so
/// what the user sees in the UI is what the validator evaluates.
pub fn parse_bitrate_to_kbps(bitrate: &str) -> Option<u32> {
    let trimmed = bitrate.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (num_part, unit_scale): (&str, f64) =
        if let Some(stripped) = trimmed.strip_suffix(['M', 'm']) {
            (stripped, 1_000.0)
        } else if let Some(stripped) = trimmed.strip_suffix(['k', 'K']) {
            (stripped, 1.0)
        } else {
            // Bare digits — interpret as bits per second.
            (trimmed, 1.0 / 1_000.0)
        };
    let parsed: f64 = num_part.trim().parse().ok()?;
    if parsed.is_nan() || parsed.is_infinite() || parsed < 0.0 {
        return None;
    }
    Some((parsed * unit_scale).round() as u32)
}

/// Reconnection configuration and state
#[derive(Debug, Clone)]
struct ReconnectionConfig {
    max_retries: u32,
    initial_delay_secs: u64,
    max_delay_secs: u64,
}

impl Default for ReconnectionConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,
            initial_delay_secs: 5,
            max_delay_secs: 120,
        }
    }
}

/// Tracks reconnection attempts for a group
#[derive(Debug, Clone)]
struct ReconnectionState {
    attempt: u32,
    last_attempt: Option<Instant>,
}

impl ReconnectionState {
    fn new() -> Self {
        Self {
            attempt: 0,
            last_attempt: None,
        }
    }

    fn increment(&mut self) {
        self.attempt += 1;
        self.last_attempt = Some(Instant::now());
    }

    fn reset(&mut self) {
        self.attempt = 0;
        self.last_attempt = None;
    }

    fn should_retry(&self, config: &ReconnectionConfig) -> bool {
        self.attempt < config.max_retries
    }

    fn next_delay(&self, config: &ReconnectionConfig) -> Duration {
        // Exponential backoff: initial * 2^attempt, capped at max_delay
        let delay_secs = config.initial_delay_secs * (1 << self.attempt.min(6));
        Duration::from_secs(delay_secs.min(config.max_delay_secs))
    }
}

// Windows: Hide console windows for spawned processes
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Placeholder used when `FFmpegLocator::discover` returns `None`.
/// `Command::new(FFMPEG_MISSING_SENTINEL)` fails with a clear OS-level
/// "ffmpeg: command not found" rather than panicking on an empty path.
const FFMPEG_MISSING_SENTINEL: &str = "ffmpeg";

/// Process info for tracking active streams
struct ProcessInfo {
    child: Child,
    start_time: Instant,
    group_id: String,
    reconnection_state: ReconnectionState,
}

/// Cached configuration for restarting groups when relay output set changes
struct ActiveGroupConfig {
    group: OutputGroup,
    incoming_url: String,
}

/// FFmpeg relay process for shared ingest
struct RelayProcess {
    child: Child,
    incoming_url: String,
    output_groups: HashSet<String>,
}

/// Manages FFmpeg streaming processes
pub struct FFmpegHandler {
    ffmpeg_path: String,
    processes: Arc<Mutex<HashMap<String, ProcessInfo>>>,
    stopping_groups: Arc<Mutex<HashSet<String>>>,
    disabled_targets: Arc<Mutex<HashSet<String>>>,
    relay: Arc<Mutex<Option<RelayProcess>>>,
    active_groups: Arc<Mutex<HashMap<String, ActiveGroupConfig>>>,
    /// Reference count for active groups using the relay
    /// Prevents race condition where relay stops while groups are still active
    relay_refcount: Arc<AtomicUsize>,
    /// Platform registry for URL normalization and redaction
    platform_registry: PlatformRegistry,
    /// Reconnection configuration
    reconnection_config: ReconnectionConfig,
    /// Port assignments for groups (group_id -> port_offset)
    /// Simplified sequential port allocation instead of hash-based
    port_assignments: Arc<Mutex<HashMap<String, u16>>>,
    /// Next available port offset for new groups
    next_port_offset: Arc<AtomicU16>,
    /// SpiritStream→OBS trigger handle. Set once at startup by
    /// `ServiceRegistry::build`. When present and the active profile's
    /// `obs.direction` allows the spiritstream→obs trigger, every
    /// successful `start_all` / `stop_all` fires a delayed OBS
    /// start/stop in core — the frontend never calls `api.obs.startStream`
    /// from its stream-control path.
    obs_trigger: std::sync::RwLock<Option<Arc<dyn ObsTrigger>>>,
}

/// Trait the OBS handler implements for the SpiritStream→OBS cascade.
/// Kept as a trait (rather than a concrete `Arc<ObsWebSocketHandler>`)
/// so `FFmpegHandler` doesn't import the entire OBS module — tests
/// can plug a mock, and any future OBS-replacement transport satisfies
/// the same surface.
#[async_trait::async_trait]
pub trait ObsTrigger: Send + Sync {
    /// Trigger OBS to start streaming. `direction_check` is true when
    /// the caller already verified the active profile's
    /// `obs.direction` allows the trigger; trait impls may still
    /// short-circuit on connection state.
    async fn trigger_start(&self);
    /// Mirror of `trigger_start` for the stop path.
    async fn trigger_stop(&self);
}

impl FFmpegHandler {
    /// Validate every output group's encoding config against the bounds the
    /// plan calls out for `StreamService::validate_config`. Returns `Ok(())`
    /// when valid; otherwise `CoreError::InvalidStreamConfig { reasons }`
    /// carrying one `ValidationIssue` per offending field. Frontends use this
    /// for decorative live feedback; the same check runs on `start()`.
    ///
    /// Rules:
    /// - At least one output group must have at least one enabled target.
    /// - Per output group with non-`copy` video:
    ///   - bitrate ∈ [500, 50000] kbps (after parsing "6000k" / "8M" / bare)
    ///   - keyframe interval ∈ [1, 10] s when set
    ///   - width and height even (FFmpeg yuv420p requires even dimensions)
    ///   - width ≥ 16 and height ≥ 16 (smallest sane stream resolution)
    ///   - fps ∈ [1, 240]
    /// - `copy` video skips encoder-specific checks — the source values
    ///   pass through unchanged.
    pub fn validate_config(profile: &Profile) -> Result<(), CoreError> {
        let mut issues: Vec<ValidationIssue> = Vec::new();

        // RTMP input must be present and configured. An RtmpInput with an empty
        // bindAddress or application can't be served — the frontend used to
        // bail at modal-render time; now the server does.
        if profile.input.bind_address.trim().is_empty() {
            issues.push(ValidationIssue {
                code: "missing_input_bind_address".into(),
                message: "Profile RTMP input has no bind address.".into(),
                path: Some("/input/bindAddress".into()),
            });
        }
        if profile.input.application.trim().is_empty() {
            issues.push(ValidationIssue {
                code: "missing_input_application".into(),
                message: "Profile RTMP input has no application path.".into(),
                path: Some("/input/application".into()),
            });
        }

        if profile.output_groups.is_empty() {
            issues.push(ValidationIssue {
                code: "no_output_groups".into(),
                message: "Profile has no output groups.".into(),
                path: Some("/outputGroups".into()),
            });
        }

        let any_target = profile
            .output_groups
            .iter()
            .any(|g| !g.stream_targets.is_empty());
        if !profile.output_groups.is_empty() && !any_target {
            issues.push(ValidationIssue {
                code: "no_stream_targets".into(),
                message: "At least one output group must have a stream target.".into(),
                path: Some("/outputGroups".into()),
            });
        }

        for (gi, group) in profile.output_groups.iter().enumerate() {
            let base = format!("/outputGroups/{gi}");
            let v = &group.video;
            let a = &group.audio;

            let is_copy_video = v.codec.eq_ignore_ascii_case("copy");
            let is_passthrough = is_copy_video && a.codec.eq_ignore_ascii_case("copy");

            // Encoder presence is the most common modal-fill error.
            if v.codec.trim().is_empty() {
                issues.push(ValidationIssue {
                    code: "group_missing_video_codec".into(),
                    message: format!("Output group '{}' has no video codec selected.", group.name),
                    path: Some(format!("{base}/video/codec")),
                });
            }

            // Codec/container compatibility. FLV (the standard RTMP container)
            // only carries H.264 video + AAC/MP3 audio — H.265/AV1/Opus inside
            // FLV is malformed and the receiving server rejects the stream
            // mid-handshake. Catch it before FFmpeg even spawns.
            let container = group.container.format.trim().to_ascii_lowercase();
            let video_codec = v.codec.trim().to_ascii_lowercase();
            let audio_codec = a.codec.trim().to_ascii_lowercase();
            if !container.is_empty() && !is_passthrough {
                let is_h264_family = video_codec.starts_with("libx264")
                    || video_codec.starts_with("h264")
                    || video_codec == "copy";
                let is_aac_or_mp3_or_copy = audio_codec.starts_with("aac")
                    || audio_codec == "libmp3lame"
                    || audio_codec == "mp3"
                    || audio_codec == "copy"
                    || audio_codec.is_empty();
                if container == "flv" {
                    if !is_h264_family && !video_codec.is_empty() {
                        issues.push(ValidationIssue {
                            code: "video_codec_container_incompatible".into(),
                            message: format!(
                                "container 'flv' only supports H.264 video; got '{}'",
                                v.codec
                            ),
                            path: Some(format!("{base}/video/codec")),
                        });
                    }
                    if !is_aac_or_mp3_or_copy {
                        issues.push(ValidationIssue {
                            code: "audio_codec_container_incompatible".into(),
                            message: format!(
                                "container 'flv' only supports AAC or MP3 audio; got '{}'",
                                a.codec
                            ),
                            path: Some(format!("{base}/audio/codec")),
                        });
                    }
                }
            }

            // Per-target checks — every target must have a destination URL
            // and a stream key. Empty values silently break the FFmpeg launch
            // mid-pipeline; failing them up-front gives the user a clear field.
            for (ti, target) in group.stream_targets.iter().enumerate() {
                let target_path_base = format!("{base}/streamTargets/{ti}");
                if target.url.trim().is_empty() {
                    issues.push(ValidationIssue {
                        code: "target_missing_url".into(),
                        message: format!("Stream target '{}' has no URL.", target.name),
                        path: Some(format!("{target_path_base}/url")),
                    });
                }
                if target.stream_key.trim().is_empty() {
                    issues.push(ValidationIssue {
                        code: "target_missing_stream_key".into(),
                        message: format!("Stream target '{}' has no stream key.", target.name),
                        path: Some(format!("{target_path_base}/streamKey")),
                    });
                }
            }

            // Resolution is only meaningful when we're re-encoding.
            // Passthrough (`copy`/`copy`) skips every encoder bound below.
            if is_passthrough {
                continue;
            }

            if !is_copy_video {
                if v.width == 0 || v.height == 0 {
                    issues.push(ValidationIssue {
                        code: "group_missing_resolution".into(),
                        message: format!("Output group '{}' has no resolution.", group.name),
                        path: Some(format!("{base}/video")),
                    });
                } else {
                    if v.width < 16 || v.height < 16 {
                        issues.push(ValidationIssue {
                            code: "video_resolution_too_small".into(),
                            message: format!(
                                "video resolution {}x{} is below the 16x16 minimum",
                                v.width, v.height
                            ),
                            path: Some(format!("{base}/video")),
                        });
                    }
                    if v.width % 2 != 0 || v.height % 2 != 0 {
                        issues.push(ValidationIssue {
                            code: "video_resolution_odd".into(),
                            message: format!(
                                "video dimensions must be even (yuv420p constraint), got {}x{}",
                                v.width, v.height
                            ),
                            path: Some(format!("{base}/video")),
                        });
                    }
                }

                match parse_bitrate_to_kbps(&v.bitrate) {
                    Some(kbps) if (VIDEO_BITRATE_MIN_KBPS..=VIDEO_BITRATE_MAX_KBPS).contains(&kbps) => {}
                    Some(kbps) => issues.push(ValidationIssue {
                        code: "video_bitrate_out_of_range".into(),
                        message: format!(
                            "video bitrate must be in [{VIDEO_BITRATE_MIN_KBPS}, {VIDEO_BITRATE_MAX_KBPS}] kbps, got {kbps}",
                        ),
                        path: Some(format!("{base}/video/bitrate")),
                    }),
                    None => issues.push(ValidationIssue {
                        code: "video_bitrate_unparseable".into(),
                        message: format!("video bitrate '{}' could not be parsed; expected '6000k' / '8M' / digits", v.bitrate),
                        path: Some(format!("{base}/video/bitrate")),
                    }),
                }

                if v.fps < FPS_MIN || v.fps > FPS_MAX {
                    issues.push(ValidationIssue {
                        code: "video_fps_out_of_range".into(),
                        message: format!(
                            "video fps must be in [{FPS_MIN}, {FPS_MAX}], got {}",
                            v.fps
                        ),
                        path: Some(format!("{base}/video/fps")),
                    });
                }

                if let Some(k) = v.keyframe_interval_seconds {
                    if !(KEYFRAME_INTERVAL_MIN_SECS..=KEYFRAME_INTERVAL_MAX_SECS).contains(&k) {
                        issues.push(ValidationIssue {
                            code: "video_keyframe_interval_out_of_range".into(),
                            message: format!(
                                "keyframe interval must be in [{KEYFRAME_INTERVAL_MIN_SECS}, {KEYFRAME_INTERVAL_MAX_SECS}] seconds, got {k}",
                            ),
                            path: Some(format!("{base}/video/keyframeIntervalSeconds")),
                        });
                    }
                }
            }
        }

        if issues.is_empty() {
            Ok(())
        } else {
            Err(CoreError::InvalidStreamConfig { reasons: issues })
        }
    }

    /// Validate a single output group's encoding config + targets — the
    /// subset of `validate_config` that's relevant when `start()` runs.
    /// Returns `CoreError::InvalidStreamConfig` with one issue per offending
    /// field. Called inside `start_internal` / `start_all_internal` so the
    /// frontend's `POST /streams/validate` call is decorative, not load-bearing.
    pub fn validate_output_group(group: &OutputGroup) -> Result<(), CoreError> {
        // Reuse the multi-group validator by wrapping in a synthetic profile
        // with a stub input (which would only fail the input-empty checks,
        // which we want to bypass here — start() validates input separately).
        let issues = Self::collect_group_issues(group, "/group");
        if issues.is_empty() {
            Ok(())
        } else {
            Err(CoreError::InvalidStreamConfig { reasons: issues })
        }
    }

    /// Extracted per-group rule set so both `validate_config` (full profile)
    /// and `validate_output_group` (single group) share the same matrix.
    fn collect_group_issues(group: &OutputGroup, base: &str) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        let v = &group.video;
        let a = &group.audio;

        let is_copy_video = v.codec.eq_ignore_ascii_case("copy");
        let is_passthrough = is_copy_video && a.codec.eq_ignore_ascii_case("copy");

        if v.codec.trim().is_empty() {
            issues.push(ValidationIssue {
                code: "group_missing_video_codec".into(),
                message: format!("Output group '{}' has no video codec selected.", group.name),
                path: Some(format!("{base}/video/codec")),
            });
        }

        for (ti, target) in group.stream_targets.iter().enumerate() {
            let target_path_base = format!("{base}/streamTargets/{ti}");
            if target.url.trim().is_empty() {
                issues.push(ValidationIssue {
                    code: "target_missing_url".into(),
                    message: format!("Stream target '{}' has no URL.", target.name),
                    path: Some(format!("{target_path_base}/url")),
                });
            }
            if target.stream_key.trim().is_empty() {
                issues.push(ValidationIssue {
                    code: "target_missing_stream_key".into(),
                    message: format!("Stream target '{}' has no stream key.", target.name),
                    path: Some(format!("{target_path_base}/streamKey")),
                });
            }
        }

        if is_passthrough {
            return issues;
        }

        if !is_copy_video {
            if v.width == 0 || v.height == 0 {
                issues.push(ValidationIssue {
                    code: "group_missing_resolution".into(),
                    message: format!("Output group '{}' has no resolution.", group.name),
                    path: Some(format!("{base}/video")),
                });
            } else {
                if v.width < 16 || v.height < 16 {
                    issues.push(ValidationIssue {
                        code: "video_resolution_too_small".into(),
                        message: format!(
                            "video resolution {}x{} is below the 16x16 minimum",
                            v.width, v.height
                        ),
                        path: Some(format!("{base}/video")),
                    });
                }
                if v.width % 2 != 0 || v.height % 2 != 0 {
                    issues.push(ValidationIssue {
                        code: "video_resolution_odd".into(),
                        message: format!(
                            "video dimensions must be even (yuv420p constraint), got {}x{}",
                            v.width, v.height
                        ),
                        path: Some(format!("{base}/video")),
                    });
                }
            }

            match parse_bitrate_to_kbps(&v.bitrate) {
                Some(kbps) if (VIDEO_BITRATE_MIN_KBPS..=VIDEO_BITRATE_MAX_KBPS).contains(&kbps) => {}
                Some(kbps) => issues.push(ValidationIssue {
                    code: "video_bitrate_out_of_range".into(),
                    message: format!(
                        "video bitrate must be in [{VIDEO_BITRATE_MIN_KBPS}, {VIDEO_BITRATE_MAX_KBPS}] kbps, got {kbps}",
                    ),
                    path: Some(format!("{base}/video/bitrate")),
                }),
                None => issues.push(ValidationIssue {
                    code: "video_bitrate_unparseable".into(),
                    message: format!("video bitrate '{}' could not be parsed; expected '6000k' / '8M' / digits", v.bitrate),
                    path: Some(format!("{base}/video/bitrate")),
                }),
            }

            if v.fps < FPS_MIN || v.fps > FPS_MAX {
                issues.push(ValidationIssue {
                    code: "video_fps_out_of_range".into(),
                    message: format!("video fps must be in [{FPS_MIN}, {FPS_MAX}], got {}", v.fps),
                    path: Some(format!("{base}/video/fps")),
                });
            }

            if let Some(k) = v.keyframe_interval_seconds {
                if !(KEYFRAME_INTERVAL_MIN_SECS..=KEYFRAME_INTERVAL_MAX_SECS).contains(&k) {
                    issues.push(ValidationIssue {
                        code: "video_keyframe_interval_out_of_range".into(),
                        message: format!(
                            "keyframe interval must be in [{KEYFRAME_INTERVAL_MIN_SECS}, {KEYFRAME_INTERVAL_MAX_SECS}] seconds, got {k}",
                        ),
                        path: Some(format!("{base}/video/keyframeIntervalSeconds")),
                    });
                }
            }
        }

        issues
    }

    // Simplified sequential port allocation for relay fan-out
    // Since only one profile is active at a time, we can use simple sequential ports
    // Each output group gets: relay_port = BASE + (index * 2), meter_port = BASE + (index * 2) + 1
    const RELAY_HOST: &'static str = "localhost";
    const RELAY_PORT_BASE: u16 = 20000; // First group: 20000 (relay), 20001 (meter)
    const RELAY_TCP_OUT_QUERY: &'static str = "tcp_nodelay=1";
    const RELAY_TCP_IN_QUERY: &'static str = "listen=1&tcp_nodelay=1";
    const RELAY_RTMP_TIMEOUT_SECS: u32 = 604_800;
    const RELAY_RTMP_TCP_NODELAY: &'static str = "1";
    const RELAY_TEE_FIFO_OPTIONS: &'static str =
        "fifo_format=mpegts:queue_size=512:drop_pkts_on_overflow=1:attempt_recovery=1:recover_any_error=1";
    const METER_HOST: &'static str = "127.0.0.1";
    const METER_UDP_QUERY: &'static str = "pkt_size=1316";

    /// Parse Windows error codes and FFmpeg error codes from log lines
    fn parse_error_details(lines: &VecDeque<String>) -> Option<String> {
        for line in lines.iter().rev() {
            // Windows socket error codes
            if line.contains("10054") {
                return Some(
                    "Connection reset by remote server (10054 - WSAECONNRESET)".to_string(),
                );
            }
            if line.contains("10053") {
                return Some("Connection aborted by network (10053 - WSAECONNABORTED)".to_string());
            }
            if line.contains("10060") {
                return Some("Connection timed out (10060 - WSAETIMEDOUT)".to_string());
            }
            if line.contains("10061") {
                return Some("Connection refused by server (10061 - WSAECONNREFUSED)".to_string());
            }
            if line.contains("10065") {
                return Some("No route to host (10065 - WSAEHOSTUNREACH)".to_string());
            }

            // FFmpeg error codes
            if line.contains("error code: -5") || line.contains("code -5") {
                return Some("I/O error (-5 - EIO): Network connection lost".to_string());
            }
            if line.contains("Connection refused") {
                return Some("RTMP server refused connection".to_string());
            }
            if line.contains("Connection timed out") {
                return Some("RTMP server connection timed out".to_string());
            }
            if line.contains("error muxing packet") {
                return Some(
                    "Failed to send packet to server (possible network issue)".to_string(),
                );
            }
        }
        None
    }

    /// Create FFmpegHandler with optional custom FFmpeg path from settings.
    ///
    /// The path is resolved at construction by `FFmpegLocator::discover`,
    /// which applies the deterministic per-deployment order: explicit
    /// `custom_path` arg → `SPIRITSTREAM_FFMPEG_PATH` env (Tauri shell
    /// bundled sidecar) → `$PATH` lookup (Linux distro / Docker).
    /// If discovery fails the handler keeps the canonical missing-path
    /// string (`ffmpeg`) so spawn calls surface "FFmpeg missing" cleanly
    /// rather than crashing on empty.
    pub fn new_with_custom_path(_app_data_dir: PathBuf, custom_path: Option<String>) -> Self {
        use crate::services::{FFmpegLocator, SettingsManager};
        // The locator's settings branch consults a real settings file —
        // emulate the same shape here by overlaying the explicit
        // `custom_path` arg first, then falling through to env/PATH.
        let resolved = if let Some(p) = custom_path.as_deref().filter(|s| !s.is_empty()) {
            if std::path::Path::new(p).exists() {
                log::info!("Using custom FFmpeg path: {p}");
                Some(std::path::PathBuf::from(p))
            } else {
                log::warn!(
                    "Custom FFmpeg path {p} does not exist; refusing to fall through silently"
                );
                None
            }
        } else {
            FFmpegLocator::discover(None::<&SettingsManager>)
        };

        let ffmpeg_path = resolved
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| FFMPEG_MISSING_SENTINEL.to_string());

        Self {
            ffmpeg_path,
            processes: Arc::new(Mutex::new(HashMap::new())),
            stopping_groups: Arc::new(Mutex::new(HashSet::new())),
            disabled_targets: Arc::new(Mutex::new(HashSet::new())),
            relay: Arc::new(Mutex::new(None)),
            active_groups: Arc::new(Mutex::new(HashMap::new())),
            relay_refcount: Arc::new(AtomicUsize::new(0)),
            platform_registry: PlatformRegistry::new(),
            reconnection_config: ReconnectionConfig::default(),
            port_assignments: Arc::new(Mutex::new(HashMap::new())),
            next_port_offset: Arc::new(AtomicU16::new(0)),
            obs_trigger: std::sync::RwLock::new(None),
        }
    }

    /// Install the SpiritStream→OBS trigger handle. Called once at
    /// startup by `ServiceRegistry::build` once the OBS handler exists.
    pub fn set_obs_trigger(&self, trigger: Arc<dyn ObsTrigger>) {
        if let Ok(mut guard) = self.obs_trigger.write() {
            *guard = Some(trigger);
        }
    }

    fn fire_obs_trigger(&self, start: bool) {
        let trigger = match self.obs_trigger.read() {
            Ok(g) => g.clone(),
            Err(_) => None,
        };
        let Some(trigger) = trigger else { return };
        // The trigger runs after a small delay so SpiritStream can
        // settle its own pipeline first. Spawned on the runtime so the
        // synchronous `start_all` / `stop_all` callers don't block.
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
            if start {
                trigger.trigger_start().await;
            } else {
                trigger.trigger_stop().await;
            }
        });
    }

    /// Normalize an RTMP URL for consistency
    fn normalize_rtmp_url(url: &str) -> String {
        let mut url = url.trim().to_string();

        // Remove trailing slashes
        while url.ends_with('/') {
            url.pop();
        }

        // Ensure rtmp:// or rtmps:// prefix if missing
        if !url.starts_with("rtmp://") && !url.starts_with("rtmps://") {
            // Check if it looks like it should be rtmps (common rtmps ports or hosts)
            if url.contains(":443") || url.contains("facebook.com") {
                url = format!("rtmps://{url}");
            } else {
                url = format!("rtmp://{url}");
            }
        }

        url
    }

    /// Sanitize a single argument with platform context for accurate redaction
    fn sanitize_arg_with_context(&self, arg: &str, group: &OutputGroup) -> String {
        if !(arg.contains("rtmp://") || arg.contains("rtmps://")) {
            return arg.to_string();
        }

        let mut parts = Vec::new();
        for segment in arg.split('|') {
            let redacted =
                if let Some(pos) = segment.find("rtmp://").or_else(|| segment.find("rtmps://")) {
                    let prefix = &segment[..pos];
                    let url_start = pos;
                    let url_end = segment[url_start..]
                        .find(' ')
                        .map(|i| url_start + i)
                        .unwrap_or(segment.len());
                    let url = &segment[url_start..url_end];
                    let suffix = &segment[url_end..];

                    // Try to find matching target to get platform
                    let platform_redacted = group
                        .stream_targets
                        .iter()
                        .find(|target| {
                            // Check if this URL belongs to this target by matching the base URL
                            let normalized = Self::normalize_rtmp_url(&target.url);
                            url.starts_with(&normalized) || url.contains(&target.url)
                        })
                        .and_then(|target| {
                            // Use platform-specific redaction and prefix
                            // with the platform's display name so operator
                            // logs are scannable when several targets are
                            // running in parallel.
                            self.platform_registry.get(&target.service).map(|config| {
                                format!("[{}] {}", config.display_name(), config.redact_url(url))
                            })
                        });

                    let redacted_url = platform_redacted.unwrap_or_else(|| {
                        // Fallback to generic redaction
                        PlatformRegistry::generic_redact(url)
                    });

                    format!("{prefix}{redacted_url}{suffix}")
                } else {
                    segment.to_string()
                };
            parts.push(redacted);
        }

        parts.join("|")
    }

    /// Sanitize all FFmpeg arguments (redact stream keys) with platform-aware redaction
    fn sanitize_ffmpeg_args(&self, args: &[String], group: &OutputGroup) -> Vec<String> {
        args.iter()
            .map(|arg| self.sanitize_arg_with_context(arg, group))
            .collect()
    }

    /// Static version of sanitize_arg for use in background threads
    /// Uses generic platform-agnostic redaction
    fn sanitize_arg_static(arg: &str) -> String {
        if !(arg.contains("rtmp://") || arg.contains("rtmps://")) {
            return arg.to_string();
        }

        let mut parts = Vec::new();
        for segment in arg.split('|') {
            let redacted = if let Some(pos) = segment.find("rtmp://") {
                let prefix = &segment[..pos];
                let url_start = pos;
                let url_end = segment[url_start..]
                    .find(' ')
                    .map(|i| url_start + i)
                    .unwrap_or(segment.len());
                let url = &segment[url_start..url_end];
                let suffix = &segment[url_end..];
                format!("{prefix}{}{suffix}", PlatformRegistry::generic_redact(url))
            } else if let Some(pos) = segment.find("rtmps://") {
                let prefix = &segment[..pos];
                let url_start = pos;
                let url_end = segment[url_start..]
                    .find(' ')
                    .map(|i| url_start + i)
                    .unwrap_or(segment.len());
                let url = &segment[url_start..url_end];
                let suffix = &segment[url_end..];
                format!("{prefix}{}{suffix}", PlatformRegistry::generic_redact(url))
            } else {
                segment.to_string()
            };
            parts.push(redacted);
        }

        parts.join("|")
    }

    fn record_active_group(
        &self,
        group: &OutputGroup,
        incoming_url: &str,
    ) -> Result<(), CoreError> {
        let mut active = self.active_groups.lock().map_err(lock_poisoned)?;

        if let Some(existing) = active.values().next() {
            if existing.incoming_url != incoming_url {
                return Err(ffmpeg_internal("Incoming URL differs from active groups"));
            }
        }

        active.insert(
            group.id.clone(),
            ActiveGroupConfig {
                group: group.clone(),
                incoming_url: incoming_url.to_string(),
            },
        );

        Ok(())
    }

    fn remove_active_group(&self, group_id: &str) {
        if let Ok(mut active) = self.active_groups.lock() {
            active.remove(group_id);
        }
    }

    fn collect_active_group_ids(&self) -> Result<HashSet<String>, CoreError> {
        let active = self.active_groups.lock().map_err(lock_poisoned)?;
        Ok(active.keys().cloned().collect())
    }

    fn resolve_active_incoming_url(&self) -> Result<String, CoreError> {
        let active = self.active_groups.lock().map_err(lock_poisoned)?;
        let mut incoming_url: Option<String> = None;
        for cfg in active.values() {
            if let Some(existing) = &incoming_url {
                if existing != &cfg.incoming_url {
                    return Err(ffmpeg_internal("Incoming URL differs across active groups"));
                }
            } else {
                incoming_url = Some(cfg.incoming_url.clone());
            }
        }
        incoming_url.ok_or_else(|| ffmpeg_internal("No active groups available"))
    }

    fn get_group_pid(&self, group_id: &str) -> Option<u32> {
        self.processes
            .lock()
            .ok()
            .and_then(|procs| procs.get(group_id).map(|info| info.child.id()))
    }

    fn relay_needs_restart(&self, desired_group_ids: &HashSet<String>) -> Result<bool, CoreError> {
        let relay_guard = self.relay.lock().map_err(lock_poisoned)?;
        if let Some(relay) = relay_guard.as_ref() {
            if !relay.output_groups.is_superset(desired_group_ids) {
                let processes = self.processes.lock().map_err(lock_poisoned)?;
                return Ok(!processes.is_empty());
            }
        }
        Ok(false)
    }

    fn is_relay_active(&self) -> Result<bool, CoreError> {
        let mut relay_guard = self.relay.lock().map_err(lock_poisoned)?;
        if let Some(relay) = relay_guard.as_mut() {
            if let Ok(Some(_)) = relay.child.try_wait() {
                *relay_guard = None;
                return Ok(false);
            }
            return Ok(true);
        }
        Ok(false)
    }

    fn stop_group_for_restart(&self, group_id: &str) -> Result<(), CoreError> {
        if let Ok(mut stopping) = self.stopping_groups.lock() {
            stopping.insert(group_id.to_string());
        }
        let removed = {
            let mut processes = self.processes.lock().map_err(lock_poisoned)?;
            processes.remove(group_id)
        };

        if let Some(mut info) = removed {
            self.stop_child(&mut info.child);
        }
        Ok(())
    }

    fn start_group_process(
        &self,
        group: &OutputGroup,
        event_sink: Arc<dyn EventSink>,
    ) -> Result<u32, CoreError> {
        let args = self.build_args(group);
        let sanitized = self.sanitize_ffmpeg_args(&args, group);
        log::info!(
            "Starting FFmpeg group {}: {} {}",
            group.id,
            self.ffmpeg_path,
            sanitized.join(" ")
        );

        let mut cmd = Command::new(&self.ffmpeg_path);
        cmd.args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        let mut child = cmd.spawn().map_err(|_| CoreError::FfmpegNotFound)?;

        let pid = child.id();
        let group_id = group.id.clone();

        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| ffmpeg_internal("Failed to capture FFmpeg stderr"))?;

        {
            let mut processes = self.processes.lock().map_err(lock_poisoned)?;
            processes.insert(
                group_id.clone(),
                ProcessInfo {
                    child,
                    start_time: Instant::now(),
                    group_id: group_id.clone(),
                    reconnection_state: ReconnectionState::new(),
                },
            );
        }

        self.relay_refcount.fetch_add(1, Ordering::SeqCst);

        let event_sink_clone = Arc::clone(&event_sink);
        let processes_clone = Arc::clone(&self.processes);
        let meter_bytes = self.start_bitrate_meter(&group_id, Arc::clone(&processes_clone));
        let relay_clone = Arc::clone(&self.relay);
        let stopping_clone = Arc::clone(&self.stopping_groups);
        let relay_refcount_clone = Arc::clone(&self.relay_refcount);
        let port_assignments_clone = Arc::clone(&self.port_assignments);
        let next_port_offset_clone = Arc::clone(&self.next_port_offset);
        let group_id_clone = group_id.clone();

        thread::spawn(move || {
            Self::stats_reader(
                stderr,
                group_id_clone,
                meter_bytes,
                event_sink_clone,
                processes_clone,
                stopping_clone,
                relay_clone,
                relay_refcount_clone,
                port_assignments_clone,
                next_port_offset_clone,
            );
        });

        Ok(pid)
    }

    fn restart_relay_with_groups(
        &self,
        requested_group_id: &str,
        event_sink: Arc<dyn EventSink>,
    ) -> Result<u32, CoreError> {
        let incoming_url = self.resolve_active_incoming_url()?;
        let desired_group_ids = self.collect_active_group_ids()?;

        let running_group_ids = self.get_active_group_ids();
        for group_id in running_group_ids {
            let _ = self.stop_group_for_restart(&group_id);
        }
        self.stop_relay();

        let active_groups = self.active_groups.lock().map_err(lock_poisoned)?;
        let mut requested_pid: Option<u32> = None;
        for (group_id, cfg) in active_groups.iter() {
            let pid = self.start_group_process(&cfg.group, Arc::clone(&event_sink))?;
            if group_id == requested_group_id {
                requested_pid = Some(pid);
            }
        }

        self.ensure_relay_running(&incoming_url, &desired_group_ids)?;

        requested_pid.ok_or_else(|| ffmpeg_internal("Requested group not started"))
    }

    /// Start streaming for an output group with stats monitoring
    pub fn start(
        &self,
        group: &OutputGroup,
        incoming_url: &str,
        event_sink: Arc<dyn EventSink>,
    ) -> Result<u32, CoreError> {
        // Authoritative encoding-config validation. Frontend's
        // `POST /streams/validate` is decorative — this is the gate that
        // refuses a malformed config before any FFmpeg process is spawned.
        Self::validate_output_group(group)?;

        self.record_active_group(group, incoming_url)?;

        if let Some(pid) = self.get_group_pid(&group.id) {
            return Ok(pid);
        }

        let desired_group_ids = self.collect_active_group_ids()?;
        if self.relay_needs_restart(&desired_group_ids)? {
            return self.restart_relay_with_groups(&group.id, event_sink);
        }

        if !self.is_relay_active()? {
            let pid = self.start_group_process(group, Arc::clone(&event_sink))?;
            self.ensure_relay_running(incoming_url, &desired_group_ids)?;
            return Ok(pid);
        }

        self.ensure_relay_running(incoming_url, &desired_group_ids)?;
        self.start_group_process(group, event_sink)
    }

    /// Start streaming for multiple output groups in one batch
    pub fn start_all(
        &self,
        groups: &[OutputGroup],
        incoming_url: &str,
        event_sink: Arc<dyn EventSink>,
    ) -> Result<Vec<u32>, CoreError> {
        // Validate every group up-front so we don't start half the stream
        // pipeline before a later group fails its bounds check. Collect all
        // failures into one InvalidStreamConfig with a `reasons` array.
        let mut all_issues = Vec::new();
        for (gi, group) in groups.iter().enumerate() {
            if group.stream_targets.is_empty() {
                continue;
            }
            let issues = Self::collect_group_issues(group, &format!("/groups/{gi}"));
            all_issues.extend(issues);
        }
        if !all_issues.is_empty() {
            return Err(CoreError::InvalidStreamConfig {
                reasons: all_issues,
            });
        }

        if self.active_count() > 0 {
            return Err(ffmpeg_internal("Streams already running"));
        }

        if let Ok(mut active) = self.active_groups.lock() {
            active.clear();
        }

        let mut desired_group_ids: HashSet<String> = HashSet::new();
        let mut start_groups: Vec<OutputGroup> = Vec::new();
        for group in groups {
            if group.stream_targets.is_empty() {
                continue;
            }
            self.record_active_group(group, incoming_url)?;
            desired_group_ids.insert(group.id.clone());
            start_groups.push(group.clone());
        }

        if start_groups.is_empty() {
            return Err(ffmpeg_internal("At least one stream target is required"));
        }

        let mut pids = Vec::with_capacity(start_groups.len());
        for group in &start_groups {
            let pid = self.start_group_process(group, Arc::clone(&event_sink))?;
            pids.push(pid);
        }

        self.ensure_relay_running(incoming_url, &desired_group_ids)?;

        // SpiritStream→OBS cascade. The trigger checks the active
        // profile's `obs.direction` server-side; this call is a no-op
        // when the trigger handle isn't installed (tests / CLI) or
        // when direction forbids it.
        self.fire_obs_trigger(true);

        Ok(pids)
    }

    fn start_bitrate_meter(
        &self,
        group_id: &str,
        processes: Arc<Mutex<HashMap<String, ProcessInfo>>>,
    ) -> Option<Arc<AtomicU64>> {
        let port = self.meter_port_for_group(group_id);
        let bind_addr = format!("{}:{}", Self::METER_HOST, port);
        let socket = match UdpSocket::bind(&bind_addr) {
            Ok(socket) => socket,
            Err(err) => {
                log::warn!(
                    "Failed to bind bitrate meter for group {group_id} on {bind_addr}: {err}"
                );
                return None;
            }
        };

        if let Err(err) = socket.set_read_timeout(Some(Duration::from_millis(250))) {
            log::warn!(
                "Failed to set meter read timeout for group {group_id} on {bind_addr}: {err}"
            );
        }

        let bytes = Arc::new(AtomicU64::new(0));
        let bytes_clone = Arc::clone(&bytes);
        let group_id = group_id.to_string();

        thread::spawn(move || {
            let mut buffer = [0u8; 2048];
            loop {
                match socket.recv_from(&mut buffer) {
                    Ok((len, _)) => {
                        bytes_clone.fetch_add(len as u64, Ordering::Relaxed);
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::TimedOut => {}
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(_) => break,
                }

                if let Ok(procs) = processes.lock() {
                    if !procs.contains_key(&group_id) {
                        break;
                    }
                } else {
                    break;
                }
            }
        });

        Some(bytes)
    }

    /// Background thread that reads FFmpeg stderr and emits stats events
    #[allow(clippy::too_many_arguments)]
    fn stats_reader(
        stderr: std::process::ChildStderr,
        group_id: String,
        meter_bytes: Option<Arc<AtomicU64>>,
        event_sink: Arc<dyn EventSink>,
        processes: Arc<Mutex<HashMap<String, ProcessInfo>>>,
        stopping_groups: Arc<Mutex<HashSet<String>>>,
        relay: Arc<Mutex<Option<RelayProcess>>>,
        relay_refcount: Arc<AtomicUsize>,
        port_assignments: Arc<Mutex<HashMap<String, u16>>>,
        next_port_offset: Arc<AtomicU16>,
    ) {
        let reader = BufReader::new(stderr);
        let mut stats = StreamStats::new(group_id.clone());
        let mut last_emit = Instant::now();
        let emit_interval = Duration::from_millis(1000); // Emit every second
        let mut was_intentionally_stopped = false;
        let mut recent_lines: VecDeque<String> = VecDeque::with_capacity(40);
        let mut last_meter_bytes = meter_bytes
            .as_ref()
            .map(|bytes| bytes.load(Ordering::Relaxed))
            .unwrap_or(0);
        let mut last_meter_instant = Instant::now();
        let mut has_meter_sample = false;
        let mut smoothed_bitrate = 0.0;
        let mut has_smoothed_bitrate = false;

        for line in reader.lines().map_while(Result::ok) {
            // Check if process is still running (was it intentionally stopped?)
            {
                if let Ok(stopping) = stopping_groups.lock() {
                    if stopping.contains(&group_id) {
                        was_intentionally_stopped = true;
                        break;
                    }
                }
                if let Ok(procs) = processes.lock() {
                    if !procs.contains_key(&group_id) {
                        // Process was removed by stop() - intentional stop
                        was_intentionally_stopped = true;
                        break;
                    }
                }
            }

            let sanitized_line = Self::sanitize_arg_static(&line);
            if recent_lines.len() == 40 {
                recent_lines.pop_front();
            }
            recent_lines.push_back(sanitized_line.clone());

            let parsed = stats.parse_line(&line);
            let is_progress_line = line.trim_start().starts_with("progress=");

            // Emit stats at most every second or at progress boundaries
            if is_progress_line || (parsed && last_emit.elapsed() >= emit_interval) {
                // Add uptime from process start if FFmpeg doesn't report time
                if let Ok(procs) = processes.lock() {
                    if let Some(info) = procs.get(&group_id) {
                        let uptime = info.start_time.elapsed().as_secs_f64();
                        if stats.time <= 0.0 {
                            stats.time = uptime;
                        }
                    }
                }

                if meter_bytes.is_none()
                    && stats.bitrate == 0.0
                    && stats.size > 0
                    && stats.time > 0.0
                {
                    let avg_kbps = (stats.size as f64 * 8.0) / 1000.0 / stats.time;
                    if avg_kbps.is_finite() && avg_kbps > 0.0 {
                        stats.bitrate = avg_kbps;
                    }
                }

                if let Some(bytes) = meter_bytes.as_ref() {
                    let now = Instant::now();
                    let current_bytes = bytes.load(Ordering::Relaxed);
                    if has_meter_sample {
                        let elapsed = now.duration_since(last_meter_instant).as_secs_f64();
                        let delta_bytes = current_bytes.saturating_sub(last_meter_bytes);
                        if elapsed > 0.0 {
                            let kbps = (delta_bytes as f64 * 8.0) / 1000.0 / elapsed;
                            if kbps.is_finite() {
                                let alpha = 0.2;
                                if has_smoothed_bitrate {
                                    smoothed_bitrate =
                                        smoothed_bitrate * (1.0 - alpha) + kbps * alpha;
                                } else {
                                    smoothed_bitrate = kbps;
                                    has_smoothed_bitrate = true;
                                }
                                stats.bitrate = smoothed_bitrate;
                            } else {
                                stats.bitrate = 0.0;
                            }
                        }
                    } else {
                        has_meter_sample = true;
                    }
                    last_meter_bytes = current_bytes;
                    last_meter_instant = now;
                }

                // Emit event
                emit_event(event_sink.as_ref(), "stream_stats", &stats);
                last_emit = Instant::now();
            }

            // Only log errors and warnings (not frame stats which are too verbose)
            if line.contains("[error]")
                || line.contains("[warning]")
                || line.contains("Error")
                || line.contains("error")
            {
                log::warn!("[FFmpeg:{group_id}] {sanitized_line}");
            }
        }

        // Decrement relay reference count when group ends
        relay_refcount.fetch_sub(1, Ordering::SeqCst);

        // Process ended - check if it was intentional or a crash
        if was_intentionally_stopped {
            if let Ok(mut stopping) = stopping_groups.lock() {
                stopping.remove(&group_id);
            }
            // Intentional stop via stop() - process already removed
            emit_event(event_sink.as_ref(), "stream_ended", &group_id);
        } else {
            // Process ended unexpectedly (crash, connection loss, etc.)
            // Remove from HashMap and check exit status
            let exit_status = {
                if let Ok(mut procs) = processes.lock() {
                    if let Some(mut info) = procs.remove(&group_id) {
                        // Free the port assignment for this crashed group
                        if let Ok(mut assignments) = port_assignments.lock() {
                            if let Some(offset) = assignments.remove(&group_id) {
                                log::debug!(
                                    "Freed port offset {offset} from crashed group {group_id}"
                                );
                                // Reset counter if all assignments are freed
                                if assignments.is_empty() {
                                    next_port_offset.store(0, Ordering::SeqCst);
                                    log::debug!("All ports freed after crash, reset counter to 0");
                                }
                            }
                        }

                        // Try to get exit status
                        match info.child.try_wait() {
                            Ok(Some(status)) => Some(status),
                            Ok(None) => info.child.wait().ok(), // Process still running, wait for it
                            Err(_) => info.child.wait().ok(), // Error checking, try to wait anyway
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            if let Ok(mut stopping) = stopping_groups.lock() {
                if stopping.remove(&group_id) {
                    emit_event(event_sink.as_ref(), "stream_ended", &group_id);
                    return;
                }
            }

            // Parse error details from recent log lines
            let error_details = Self::parse_error_details(&recent_lines);

            // Determine if this was a crash or normal exit
            let error_message = match exit_status {
                Some(status) if status.success() => {
                    // FFmpeg exited cleanly (exit code 0)
                    // This might happen if the input stream ended
                    None
                }
                Some(status) => {
                    // FFmpeg exited with error
                    let code = status.code().unwrap_or(-1);
                    let base_msg = format!("FFmpeg exited with code {code}");

                    // Append detailed error if available
                    if let Some(details) = error_details {
                        Some(format!("{base_msg}: {details}"))
                    } else {
                        Some(base_msg)
                    }
                }
                None => {
                    // Couldn't get exit status
                    let base_msg = "FFmpeg process terminated unexpectedly".to_string();
                    if let Some(details) = error_details {
                        Some(format!("{base_msg}: {details}"))
                    } else {
                        Some(base_msg)
                    }
                }
            };

            if let Some(error) = error_message {
                log::error!("[FFmpeg:{group_id}] Stream crashed: {error}");
                if !recent_lines.is_empty() {
                    log::warn!("[FFmpeg:{group_id}] Last 10 stderr lines:");
                    for entry in recent_lines.iter().rev().take(10).rev() {
                        log::warn!("[FFmpeg:{group_id}]   {entry}");
                    }
                }

                // Emit stream_error event with group_id, error message, and reconnection hint
                emit_event(
                    event_sink.as_ref(),
                    "stream_error",
                    &serde_json::json!({
                        "groupId": group_id,
                        "error": error,
                        "canRetry": true,
                        "suggestion": "Stream connection lost. Click retry to reconnect automatically."
                    }),
                );
            } else {
                // Clean exit (input ended)
                emit_event(event_sink.as_ref(), "stream_ended", &group_id);
            }
        }

        // Check relay refcount and stop relay if no more groups are using it
        // Use atomic load to avoid race condition where multiple groups finish simultaneously
        let should_stop_relay = relay_refcount.load(Ordering::SeqCst) == 0;
        if should_stop_relay {
            if let Ok(procs) = processes.lock() {
                if !procs.is_empty() {
                    return;
                }
            }
            if let Ok(mut relay_guard) = relay.lock() {
                if let Some(mut relay_proc) = relay_guard.take() {
                    log::info!("Stopping relay process (no active groups)");
                    let _ = relay_proc.child.kill();
                    let _ = relay_proc.child.wait();
                }
            }
        }
    }

    /// Stop streaming for an output group
    pub fn stop(&self, group_id: &str) -> Result<(), CoreError> {
        self.remove_active_group(group_id);
        if let Ok(mut stopping) = self.stopping_groups.lock() {
            stopping.insert(group_id.to_string());
        }
        let (removed, should_stop_relay) = {
            let mut processes = self.processes.lock().map_err(lock_poisoned)?;
            let removed = processes.remove(group_id);
            let should_stop_relay = processes.is_empty();
            (removed, should_stop_relay)
        };

        if let Some(mut info) = removed {
            self.stop_child(&mut info.child);
            // Free the port assignment for this group
            self.free_port_offset(group_id);
        }

        // Only stop relay when ALL groups are stopped
        // Don't restart relay when stopping individual groups - this would interrupt
        // the input for remaining groups and cause them to fail
        if should_stop_relay {
            self.stop_relay();
        }

        Ok(())
    }

    /// Stop all active streams
    pub fn stop_all(&self) -> Result<(), CoreError> {
        if let Ok(mut active) = self.active_groups.lock() {
            active.clear();
        }
        let mut processes = self.processes.lock().map_err(lock_poisoned)?;
        let mut stopping = self.stopping_groups.lock().map_err(lock_poisoned)?;
        for (group_id, mut info) in processes.drain() {
            stopping.insert(group_id);
            self.stop_child(&mut info.child);
        }
        self.stop_relay();

        // Clear all port assignments since all groups are stopped
        if let Ok(mut assignments) = self.port_assignments.lock() {
            assignments.clear();
            self.next_port_offset.store(0, Ordering::SeqCst);
            log::debug!("All groups stopped, cleared all port assignments");
        }

        // SpiritStream→OBS cascade — see `start_all` for the mirror.
        self.fire_obs_trigger(false);

        Ok(())
    }

    /// Get active stream count
    pub fn active_count(&self) -> usize {
        self.processes.lock().map(|procs| procs.len()).unwrap_or(0)
    }

    /// Check if a group is streaming
    pub fn is_streaming(&self, group_id: &str) -> bool {
        self.processes
            .lock()
            .map(|procs| procs.contains_key(group_id))
            .unwrap_or(false)
    }

    /// Get list of active stream group IDs
    pub fn get_active_group_ids(&self) -> Vec<String> {
        self.processes
            .lock()
            .map(|procs| procs.values().map(|info| info.group_id.clone()).collect())
            .unwrap_or_default()
    }

    /// Enable a specific stream target (removes from disabled set)
    pub fn enable_target(&self, target_id: &str) {
        let mut disabled = self.disabled_targets.lock().unwrap_or_else(|e| {
            log::warn!("Disabled targets mutex poisoned (enable_target), recovering: {e}");
            e.into_inner()
        });
        disabled.remove(target_id);
    }

    /// Disable a specific stream target (adds to disabled set)
    pub fn disable_target(&self, target_id: &str) {
        let mut disabled = self.disabled_targets.lock().unwrap_or_else(|e| {
            log::warn!("Disabled targets mutex poisoned (disable_target), recovering: {e}");
            e.into_inner()
        });
        disabled.insert(target_id.to_string());
    }

    /// Check if a target is currently disabled
    pub fn is_target_disabled(&self, target_id: &str) -> bool {
        let disabled = self.disabled_targets.lock().unwrap_or_else(|e| {
            log::warn!("Disabled targets mutex poisoned (is_target_disabled), recovering: {e}");
            e.into_inner()
        });
        disabled.contains(target_id)
    }

    /// Ensure relay process is running for shared ingest
    fn ensure_relay_running(
        &self,
        incoming_url: &str,
        requested_groups: &HashSet<String>,
    ) -> Result<(), CoreError> {
        let mut relay_guard = self.relay.lock().map_err(lock_poisoned)?;

        if let Some(relay) = relay_guard.as_mut() {
            if let Ok(Some(_)) = relay.child.try_wait() {
                *relay_guard = None;
            }
        }

        if let Some(relay) = relay_guard.as_ref() {
            if relay.incoming_url != incoming_url {
                return Err(ffmpeg_internal(
                    "Incoming URL differs from active relay input",
                ));
            }

            // Only skip restart if relay has EXACTLY the requested groups
            // (not just a superset, to handle group removal)
            if relay.output_groups == *requested_groups {
                return Ok(());
            }
        }

        if requested_groups.is_empty() {
            return Err(ffmpeg_internal(
                "No output groups provided for relay fan-out",
            ));
        }

        // Use the exact requested groups list (don't extend existing)
        let relay_groups = requested_groups.clone();

        if let Some(mut relay) = relay_guard.take() {
            let _ = relay.child.kill();
            let _ = relay.child.wait();
        }

        let args = self.build_relay_args(incoming_url, &relay_groups)?;
        let sanitized: Vec<String> = args
            .iter()
            .map(|arg| Self::sanitize_arg_static(arg))
            .collect();
        log::info!(
            "Starting FFmpeg relay: {} {}",
            self.ffmpeg_path,
            sanitized.join(" ")
        );
        let mut cmd = Command::new(&self.ffmpeg_path);
        cmd.args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        let mut child = cmd.spawn().map_err(|_| CoreError::FfmpegNotFound)?;

        if let Some(stderr) = child.stderr.take() {
            thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    let sanitized = Self::sanitize_arg_static(&line);
                    if line.contains("[error]")
                        || line.contains("[warning]")
                        || line.contains("Error")
                        || line.contains("error")
                        || line.contains("Failed")
                        || line.contains("failed")
                        || line.contains("Connection")
                        || line.contains("connection")
                        || line.contains("listen")
                    {
                        log::warn!("[FFmpeg:relay] {sanitized}");
                    }
                }
            });
        }

        *relay_guard = Some(RelayProcess {
            child,
            incoming_url: incoming_url.to_string(),
            output_groups: relay_groups,
        });

        Ok(())
    }

    /// Stop the relay process if running
    fn stop_relay(&self) {
        let mut relay_guard = match self.relay.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };

        if let Some(mut relay) = relay_guard.take() {
            let _ = relay.child.kill();
            let _ = relay.child.wait();
        }
    }

    fn stop_child(&self, child: &mut Child) {
        if let Some(stdin) = child.stdin.as_mut() {
            let _ = stdin.write_all(b"q\n");
            let _ = stdin.flush();
        }

        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(2) {
            if let Ok(Some(_)) = child.try_wait() {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }

        let _ = child.kill();
        let _ = child.wait();
    }

    /// Restart a specific group (used after toggling targets)
    /// This stops the group and restarts it with the updated target list
    pub fn restart_group(
        &self,
        group_id: &str,
        group: &OutputGroup,
        incoming_url: &str,
        event_sink: Arc<dyn EventSink>,
    ) -> Result<u32, CoreError> {
        if self.is_streaming(group_id) {
            self.stop(group_id)?;
        }

        // Start with updated target list (disabled targets will be filtered out)
        self.start(group, incoming_url, event_sink)
    }

    /// Retry a failed group with exponential backoff
    /// Returns the delay that should be waited before the next retry
    pub fn retry_group(
        &self,
        group_id: &str,
        event_sink: Arc<dyn EventSink>,
    ) -> Result<(u32, Option<Duration>), CoreError> {
        // Get the active group configuration
        let active_groups = self.active_groups.lock().map_err(lock_poisoned)?;

        let config = active_groups
            .get(group_id)
            .ok_or_else(|| ffmpeg_internal("Group not found in active groups"))?;

        let group = config.group.clone();
        drop(active_groups);

        // Check if the group is already running
        if self.is_streaming(group_id) {
            return Err(ffmpeg_internal("Group is already streaming"));
        }

        // Get or create reconnection state
        let mut reconnection_state = {
            let processes = self.processes.lock().map_err(lock_poisoned)?;

            processes
                .get(group_id)
                .map(|info| info.reconnection_state.clone())
                .unwrap_or_else(ReconnectionState::new)
        };

        // Check if we should retry
        if !reconnection_state.should_retry(&self.reconnection_config) {
            // terminal state — emit `stream_retry_exhausted` so the
            // UI knows the group is permanently failed, and tear down ALL
            // residual state so subsequent `get_active_group_ids` /
            // `is_streaming` queries report it correctly. We have to drop
            // both the active-group config AND any dead `processes` entry
            // (the FFmpeg child already exited; the map entry is a corpse).
            emit_event(
                event_sink.as_ref(),
                "stream_retry_exhausted",
                &serde_json::json!({
                    "groupId": group_id,
                    "maxAttempts": self.reconnection_config.max_retries,
                }),
            );
            self.remove_active_group(group_id);
            if let Ok(mut processes) = self.processes.lock() {
                processes.remove(group_id);
            }
            return Err(ffmpeg_internal(format!(
                "Maximum retry attempts ({}) reached",
                self.reconnection_config.max_retries
            )));
        }

        // Calculate backoff delay
        let delay = reconnection_state.next_delay(&self.reconnection_config);

        // Log retry attempt
        log::info!(
            "[FFmpeg:{group_id}] Retrying stream (attempt {}/{}) after {} seconds",
            reconnection_state.attempt + 1,
            self.reconnection_config.max_retries,
            delay.as_secs()
        );

        // Emit reconnecting event
        emit_event(
            event_sink.as_ref(),
            "stream_retry_attempt",
            &serde_json::json!({
                "groupId": group_id,
                "attempt": reconnection_state.attempt + 1,
                "maxAttempts": self.reconnection_config.max_retries,
                "delaySecs": delay.as_secs()
            }),
        );

        // Sleep for backoff delay
        thread::sleep(delay);

        // Increment retry counter
        reconnection_state.increment();

        // Attempt to start the stream
        match self.start_group_process(&group, event_sink.clone()) {
            Ok(pid) => {
                // Success - update reconnection state in process info
                if let Ok(mut processes) = self.processes.lock() {
                    if let Some(info) = processes.get_mut(group_id) {
                        info.reconnection_state = reconnection_state.clone();
                    }
                }

                // Calculate next retry delay in case this one fails
                let next_delay = if reconnection_state.should_retry(&self.reconnection_config) {
                    Some(reconnection_state.next_delay(&self.reconnection_config))
                } else {
                    None
                };

                log::info!("[FFmpeg:{group_id}] Stream reconnected successfully (PID: {pid})");
                Ok((pid, next_delay))
            }
            Err(e) => {
                log::error!("[FFmpeg:{group_id}] Reconnection failed: {e}");
                Err(ffmpeg_internal(format!("Failed to reconnect: {e}")))
            }
        }
    }

    /// Reset reconnection state for a group (called on successful manual start)
    pub fn reset_reconnection_state(&self, group_id: &str) {
        if let Ok(mut processes) = self.processes.lock() {
            if let Some(info) = processes.get_mut(group_id) {
                info.reconnection_state.reset();
            }
        }
    }

    /// Resolve stream key - supports ${ENV_VAR} syntax
    fn resolve_stream_key(key: &str) -> String {
        // Check if key matches ${VAR_NAME} pattern
        if key.starts_with("${") && key.ends_with("}") && key.len() > 3 {
            let var_name = &key[2..key.len() - 1];
            match std::env::var(var_name) {
                Ok(value) => {
                    // Security: Do not log the variable name to prevent revealing
                    // which environment variables contain sensitive credentials
                    log::debug!("Resolved stream key from environment variable");
                    value
                }
                Err(_) => {
                    // Security: Do not log the variable name to prevent revealing
                    // credential-related configuration details
                    log::warn!(
                        "Environment variable not found for stream key, check your configuration"
                    );
                    key.to_string()
                }
            }
        } else {
            key.to_string()
        }
    }

    /// Build FFmpeg arguments for the shared relay process
    fn build_relay_args(
        &self,
        incoming_url: &str,
        group_ids: &HashSet<String>,
    ) -> Result<Vec<String>, CoreError> {
        if group_ids.is_empty() {
            return Err(ffmpeg_internal("Relay fan-out requires at least one group"));
        }

        let outputs = self.relay_tee_output_list(group_ids);
        let listen_url = Self::normalize_relay_input_url(incoming_url);
        Ok(vec![
            "-listen".to_string(),
            "1".to_string(),
            "-timeout".to_string(),
            Self::RELAY_RTMP_TIMEOUT_SECS.to_string(),
            "-tcp_nodelay".to_string(),
            Self::RELAY_RTMP_TCP_NODELAY.to_string(),
            "-i".to_string(),
            listen_url,
            "-c:v".to_string(),
            "copy".to_string(),
            "-c:a".to_string(),
            "copy".to_string(),
            "-map".to_string(),
            "0:v".to_string(),
            "-map".to_string(),
            "0:a".to_string(),
            "-f".to_string(),
            "tee".to_string(),
            "-use_fifo".to_string(),
            "1".to_string(),
            "-fifo_options".to_string(),
            Self::RELAY_TEE_FIFO_OPTIONS.to_string(),
            outputs,
        ])
    }

    /// Get or assign a port offset for a group (simplified sequential allocation)
    fn get_port_offset(&self, group_id: &str) -> u16 {
        let mut assignments = self.port_assignments.lock().unwrap_or_else(|e| {
            log::warn!("Port assignments mutex poisoned, recovering: {e}");
            e.into_inner()
        });

        // Return existing assignment if found
        if let Some(&offset) = assignments.get(group_id) {
            return offset;
        }

        // Assign next available offset, preventing AtomicU16 wraparound
        let offset = loop {
            let current = self.next_port_offset.load(Ordering::SeqCst);
            if current == u16::MAX {
                log::error!(
                    "Port offset counter reached maximum value ({}); \
                    refusing to allocate new port offset to group {group_id}",
                    u16::MAX
                );
                break current;
            }

            let next = current + 1;
            match self.next_port_offset.compare_exchange(
                current,
                next,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => break current,
                Err(_) => continue,
            }
        };
        assignments.insert(group_id.to_string(), offset);

        log::debug!(
            "Assigned port offset {offset} to group {group_id} (relay: {}, meter: {})",
            Self::RELAY_PORT_BASE + (offset * 2),
            Self::RELAY_PORT_BASE + (offset * 2) + 1
        );

        offset
    }

    /// Free port assignment when a group stops
    fn free_port_offset(&self, group_id: &str) {
        let mut assignments = self.port_assignments.lock().unwrap_or_else(|e| {
            log::warn!("Port assignments mutex poisoned, recovering: {e}");
            e.into_inner()
        });

        if let Some(offset) = assignments.remove(group_id) {
            log::debug!("Freed port offset {offset} from group {group_id}");

            // Reset counter if all assignments are freed
            if assignments.is_empty() {
                self.next_port_offset.store(0, Ordering::SeqCst);
                log::debug!("All ports freed, reset counter to 0");
            }
        }
    }

    fn relay_port_for_group(&self, group_id: &str) -> u16 {
        let offset = self.get_port_offset(group_id);
        // Each group gets: relay_port = BASE + (offset * 2)
        Self::RELAY_PORT_BASE + (offset * 2)
    }

    fn relay_output_url_for_group(&self, group_id: &str) -> String {
        format!(
            "tcp://{}:{}?{}",
            Self::RELAY_HOST,
            self.relay_port_for_group(group_id),
            Self::RELAY_TCP_OUT_QUERY
        )
    }

    fn relay_input_url_for_group(&self, group_id: &str) -> String {
        format!(
            "tcp://{}:{}?{}",
            Self::RELAY_HOST,
            self.relay_port_for_group(group_id),
            Self::RELAY_TCP_IN_QUERY
        )
    }

    fn meter_port_for_group(&self, group_id: &str) -> u16 {
        let offset = self.get_port_offset(group_id);
        // Each group gets: meter_port = BASE + (offset * 2) + 1
        Self::RELAY_PORT_BASE + (offset * 2) + 1
    }

    fn meter_output_url_for_group(&self, group_id: &str) -> String {
        format!(
            "udp://{}:{}?{}",
            Self::METER_HOST,
            self.meter_port_for_group(group_id),
            Self::METER_UDP_QUERY
        )
    }

    fn relay_tee_output_list(&self, group_ids: &HashSet<String>) -> String {
        let mut ids: Vec<&String> = group_ids.iter().collect();
        ids.sort();
        ids.into_iter()
            .map(|id| format!("[f=mpegts]{}", self.relay_output_url_for_group(id)))
            .collect::<Vec<String>>()
            .join("|")
    }

    fn normalize_relay_input_url(url: &str) -> String {
        if !(url.starts_with("rtmp://") || url.starts_with("rtmps://")) {
            return url.to_string();
        }

        let without_query = url.split('?').next().unwrap_or(url);
        let trimmed = without_query.trim_end_matches('/');

        let (scheme, rest) = match trimmed.split_once("://") {
            Some(parts) => parts,
            None => return url.to_string(),
        };

        let mut host_and_path = rest.splitn(2, '/');
        let host = match host_and_path.next() {
            Some(value) if !value.is_empty() => value,
            _ => return url.to_string(),
        };
        let host = if host == "0.0.0.0" {
            "127.0.0.1".to_string()
        } else if let Some(port) = host.strip_prefix("0.0.0.0:") {
            format!("127.0.0.1:{port}")
        } else {
            host.to_string()
        };

        let path = host_and_path.next().unwrap_or("");
        let app = path.split('/').find(|segment| !segment.is_empty());

        let base_url = if let Some(app) = app {
            format!("{scheme}://{host}/{app}")
        } else {
            format!("{scheme}://{host}")
        };

        base_url
    }

    fn double_bitrate_value(bitrate: &str) -> Option<String> {
        let trimmed = bitrate.trim();
        if trimmed.is_empty() {
            return None;
        }

        let split_at = trimmed
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(trimmed.len());
        let (value_str, suffix) = trimmed.split_at(split_at);
        if value_str.is_empty() {
            return None;
        }

        let value: f64 = value_str.parse().ok()?;
        let doubled = value * 2.0;
        let formatted = format!("{doubled}");
        Some(format!("{formatted}{suffix}"))
    }

    fn append_cbr_args(args: &mut Vec<String>, encoder: &str, bitrate: &str) {
        let bufsize = Self::double_bitrate_value(bitrate).unwrap_or_else(|| bitrate.to_string());

        args.push("-minrate".to_string());
        args.push(bitrate.to_string());
        args.push("-maxrate".to_string());
        args.push(bitrate.to_string());
        args.push("-bufsize".to_string());
        args.push(bufsize);

        if encoder.contains("nvenc") || encoder.contains("qsv") || encoder.contains("amf") {
            args.push("-rc".to_string());
            args.push("cbr".to_string());
        }

        if encoder == "libx264" {
            args.push("-x264-params".to_string());
            args.push("nal-hrd=cbr:force-cfr=1".to_string());
        } else if encoder == "libx265" {
            args.push("-x265-params".to_string());
            args.push("nal-hrd=cbr".to_string());
        }
    }

    fn map_nvenc_preset(preset: &str) -> String {
        let normalized = preset.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return "p4".to_string();
        }

        match normalized.as_str() {
            "p1" | "p2" | "p3" | "p4" | "p5" | "p6" | "p7" | "default" | "slow" | "medium"
            | "fast" | "hp" | "hq" | "bd" | "ll" | "llhq" | "llhp" | "lossless" | "losslesshp" => {
                normalized
            }
            "ultrafast" => "p1".to_string(),
            "superfast" => "p2".to_string(),
            "veryfast" => "p3".to_string(),
            "faster" => "p4".to_string(),
            "slower" => "p6".to_string(),
            "veryslow" => "p7".to_string(),
            "quality" => "p7".to_string(),
            "balanced" => "p4".to_string(),
            "performance" => "p2".to_string(),
            "low_latency" | "low-latency" | "lowlatency" => "p1".to_string(),
            _ => "p4".to_string(),
        }
    }

    /// Add RTMP protocol options to a URL for connection resilience
    ///
    /// Matches OBS Studio's RTMP configuration for maximum stability:
    /// - timeout: 30 seconds (matching OBS receive timeout)
    /// - rtmp_buffer: 30 seconds (matching OBS buffer, 10x default)
    /// - tcp_keepalive: Enabled to detect dead connections via TCP probes
    /// - rtmp_live: Live stream mode (optimizes for live rather than VOD)
    fn add_rtmp_options(url: &str) -> String {
        if !url.starts_with("rtmp://") && !url.starts_with("rtmps://") {
            return url.to_string();
        }

        // FFmpeg RTMP protocol options verified in FFmpeg documentation
        // https://ffmpeg.org/ffmpeg-protocols.html#rtmp
        let options = [
            ("timeout", "30000000"),  // 30 seconds in microseconds (receive timeout)
            ("rtmp_buffer", "30000"), // 30 seconds in milliseconds (client buffer)
            ("tcp_keepalive", "1"),   // Enable TCP keepalive probes
            ("rtmp_live", "live"),    // Live stream mode
        ];

        // Check if URL already has query parameters
        let separator = if url.contains('?') { "&" } else { "?" };

        // Build query string
        let query = options
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");

        format!("{url}{separator}{query}")
    }

    /// Build FFmpeg arguments for an output group
    ///
    /// Groups read from the shared TCP relay so they can restart independently.
    fn build_args(&self, group: &OutputGroup) -> Vec<String> {
        // Determine if we should use stream copy (passthrough mode)
        // When both video and audio codecs are set to "copy", FFmpeg acts as a pure
        // RTMP relay server, accepting the incoming stream and forwarding it to outputs
        // without re-encoding. This is the default behavior and most efficient mode.
        // Use case-insensitive comparison to handle "Copy", "COPY", etc.
        let use_stream_copy = group.video.codec.eq_ignore_ascii_case("copy")
            && group.audio.codec.eq_ignore_ascii_case("copy");

        let mut args = Vec::new();

        // Build fflags: always discard corrupt frames, optionally generate PTS
        // +discardcorrupt: silently drop corrupt frames (always good for live streaming)
        // +genpts: generate fresh presentation timestamps (helps with timing issues)
        let fflags = if group.generate_pts {
            "+discardcorrupt+genpts"
        } else {
            "+discardcorrupt"
        };
        args.push("-fflags".to_string());
        args.push(fflags.to_string());

        // Copy timestamps from input when PTS generation is enabled
        if group.generate_pts {
            args.push("-copyts".to_string());
        }

        // Input source
        args.push("-i".to_string());
        args.push(self.relay_input_url_for_group(&group.id));

        // Audio sync when PTS generation is enabled
        // -async 1: resample audio to match timestamps, fixing drift
        if group.generate_pts {
            args.push("-async".to_string());
            args.push("1".to_string());
        }

        if use_stream_copy {
            args.push("-c:v".to_string());
            args.push("copy".to_string());
            args.push("-c:a".to_string());
            args.push("copy".to_string());
        } else {
            // Video settings
            args.push("-c:v".to_string());
            args.push(group.video.codec.clone());
            args.push("-s".to_string());
            args.push(group.video.resolution());
            args.push("-b:v".to_string());
            args.push(group.video.bitrate.clone());
            // Add CBR enforcement for consistent streaming bitrate
            Self::append_cbr_args(&mut args, &group.video.codec, &group.video.bitrate);
            args.push("-r".to_string());
            args.push(group.video.fps.to_string());
            // Audio settings
            args.push("-c:a".to_string());
            args.push(group.audio.codec.clone());
            args.push("-b:a".to_string());
            args.push(group.audio.bitrate.clone());
            args.push("-ac".to_string());
            args.push(group.audio.channels.to_string());
            args.push("-ar".to_string());
            args.push(group.audio.sample_rate.to_string());
            // Add video encoder preset if specified
            if let Some(preset) = &group.video.preset {
                let encoder = group.video.codec.as_str();
                if encoder.contains("amf") {
                    let mut amf_quality: Option<&str> = None;
                    let mut amf_usage: Option<&str> = None;
                    match preset.as_str() {
                        "quality" => amf_quality = Some("quality"),
                        "balanced" => amf_quality = Some("balanced"),
                        "speed" => amf_quality = Some("speed"),
                        "performance" | "fast" | "faster" | "veryfast" | "superfast"
                        | "ultrafast" => {
                            amf_quality = Some("speed");
                        }
                        "medium" => amf_quality = Some("balanced"),
                        "slow" | "slower" | "veryslow" => amf_quality = Some("quality"),
                        "low_latency" | "low-latency" | "lowLatency" => {
                            amf_quality = Some("speed");
                            amf_usage = Some("lowlatency");
                        }
                        _ => {}
                    }
                    if let Some(quality) = amf_quality {
                        args.push("-quality".to_string());
                        args.push(quality.to_string());
                    }
                    if let Some(usage) = amf_usage {
                        args.push("-usage".to_string());
                        args.push(usage.to_string());
                    }
                } else if encoder.contains("nvenc") {
                    let ffmpeg_preset = Self::map_nvenc_preset(preset);
                    args.push("-preset".to_string());
                    args.push(ffmpeg_preset);
                } else {
                    let supports_preset = encoder == "libx264" || encoder == "libx265";
                    if supports_preset {
                        let ffmpeg_preset = match preset.as_str() {
                            "quality" => "slow",
                            "balanced" => "medium",
                            "performance" => "fast",
                            "low_latency" | "low-latency" | "lowLatency" => "ultrafast",
                            _ => preset.as_str(),
                        };
                        args.push("-preset".to_string());
                        args.push(ffmpeg_preset.to_string());
                    }
                }
            }
            // Add H.264 profile if specified
            if let Some(profile) = &group.video.profile {
                args.push("-profile:v".to_string());
                args.push(profile.clone());
            }

            if let Some(interval_seconds) = group.video.keyframe_interval_seconds {
                if interval_seconds > 0 && group.video.fps > 0 {
                    let gop_size = group.video.fps.saturating_mul(interval_seconds);
                    if gop_size > 0 {
                        args.push("-g".to_string());
                        args.push(gop_size.to_string());

                        if group.video.codec == "libx264" || group.video.codec == "libx265" {
                            args.push("-keyint_min".to_string());
                            args.push(gop_size.to_string());
                            args.push("-sc_threshold".to_string());
                            args.push("0".to_string());
                        }

                        args.push("-force_key_frames".to_string());
                        args.push(format!("expr:gte(t,n_forced*{interval_seconds})"));
                    }
                }
            }
        }

        if group.container.format == "flv" {
            let force_flv_video_tag = use_stream_copy || group.video.codec.contains("264");
            if force_flv_video_tag {
                args.push("-tag:v".to_string());
                args.push("7".to_string());
            }

            let force_flv_audio_tag = use_stream_copy || group.audio.codec.contains("aac");
            if force_flv_audio_tag {
                args.push("-tag:a".to_string());
                args.push("10".to_string());
            }

            if use_stream_copy {
                args.push("-bsf:a".to_string());
                args.push("aac_adtstoasc".to_string());
            }
        }

        // Always map video and audio from input 0
        args.push("-map".to_string());
        args.push("0:v".to_string());
        args.push("-map".to_string());
        args.push("0:a".to_string());

        // Progress output for stats parsing
        args.push("-progress".to_string());
        args.push("pipe:2".to_string());
        args.push("-stats".to_string());

        // Add output targets (skip disabled ones)
        let disabled = self.disabled_targets.lock().unwrap_or_else(|e| {
            log::warn!("Disabled targets mutex poisoned (build_args), recovering: {e}");
            e.into_inner()
        });
        let mut target_outputs: Vec<String> = Vec::new();
        for target in &group.stream_targets {
            // Skip targets that have been disabled via toggle_target
            if disabled.contains(&target.id) {
                continue;
            }

            let normalized_url = Self::normalize_rtmp_url(&target.url);
            let normalized_url = self
                .platform_registry
                .normalize_url(&target.service, &normalized_url);
            let resolved_key = Self::resolve_stream_key(&target.stream_key);
            let full_url = self.platform_registry.build_url_with_key(
                &target.service,
                &normalized_url,
                &resolved_key,
            );

            // Add RTMP protocol options for connection resilience (matches OBS configuration)
            let full_url_with_options =
                if full_url.starts_with("rtmp://") || full_url.starts_with("rtmps://") {
                    Self::add_rtmp_options(&full_url)
                } else {
                    full_url.clone()
                };
            target_outputs.push(full_url_with_options);
        }

        if target_outputs.is_empty() {
            return args;
        }

        let meter_output = self.meter_output_url_for_group(&group.id);

        // Always use onfail=ignore for RTMP outputs to prevent one failed connection
        // from killing the stats meter and potentially other outputs
        let mut tee_outputs: Vec<String> = Vec::new();
        tee_outputs.extend(
            target_outputs
                .iter()
                .map(|output| format!("[f={}:onfail=ignore]{output}", group.container.format)),
        );
        tee_outputs.push(format!("[f=mpegts:onfail=ignore]{meter_output}"));

        args.push("-f".to_string());
        args.push("tee".to_string());
        args.push(tee_outputs.join("|"));

        args
    }
}

#[cfg(test)]
mod validate_tests {
    use super::*;
    use crate::models::Platform;
    use crate::models::{
        AudioSettings, ContainerSettings, OutputGroup, Profile, ProfileSettings, RtmpInput,
        StreamTarget, VideoSettings,
    };

    fn copy_video() -> VideoSettings {
        VideoSettings {
            codec: "copy".into(),
            width: 0,
            height: 0,
            fps: 0,
            bitrate: "0k".into(),
            preset: None,
            profile: None,
            keyframe_interval_seconds: None,
        }
    }

    fn copy_audio() -> AudioSettings {
        AudioSettings {
            codec: "copy".into(),
            bitrate: "0k".into(),
            channels: 0,
            sample_rate: 0,
        }
    }

    fn target(url: &str, key: &str) -> StreamTarget {
        StreamTarget {
            id: "t-1".into(),
            name: "t1".into(),
            service: Platform::Twitch,
            url: url.into(),
            stream_key: key.into(),
        }
    }

    fn baseline_profile() -> Profile {
        Profile {
            id: "p1".into(),
            name: "p1".into(),
            encrypted: false,
            input: RtmpInput {
                input_type: "rtmp".into(),
                bind_address: "127.0.0.1".into(),
                port: 1935,
                application: "live".into(),
            },
            output_groups: vec![OutputGroup {
                id: "g1".into(),
                name: "Group 1".into(),
                is_default: true,
                generate_pts: true,
                video: copy_video(),
                audio: copy_audio(),
                container: ContainerSettings::default(),
                stream_targets: vec![target("rtmp://x/live", "key1")],
            }],
            settings: ProfileSettings::default(),
            pii_blocklist: vec![],
            pii_fuzzy: false,
            anonymous_logging: true,
            anonymous_salt: String::new(),
        }
    }

    #[test]
    fn passthrough_profile_with_target_passes() {
        let p = baseline_profile();
        FFmpegHandler::validate_config(&p).expect("baseline passthrough must validate");
    }

    #[test]
    fn missing_input_bind_address_is_flagged() {
        let mut p = baseline_profile();
        p.input.bind_address = "".into();
        match FFmpegHandler::validate_config(&p) {
            Err(CoreError::InvalidStreamConfig { reasons }) => {
                assert!(reasons
                    .iter()
                    .any(|r| r.code == "missing_input_bind_address"));
            }
            other => panic!("expected InvalidStreamConfig, got {other:?}"),
        }
    }

    #[test]
    fn missing_input_application_is_flagged() {
        let mut p = baseline_profile();
        p.input.application = "".into();
        match FFmpegHandler::validate_config(&p) {
            Err(CoreError::InvalidStreamConfig { reasons }) => {
                assert!(reasons
                    .iter()
                    .any(|r| r.code == "missing_input_application"));
            }
            other => panic!("expected InvalidStreamConfig, got {other:?}"),
        }
    }

    #[test]
    fn empty_output_groups_is_flagged() {
        let mut p = baseline_profile();
        p.output_groups.clear();
        let err = FFmpegHandler::validate_config(&p).unwrap_err();
        match err {
            CoreError::InvalidStreamConfig { reasons } => {
                assert!(reasons.iter().any(|r| r.code == "no_output_groups"));
            }
            other => panic!("expected InvalidStreamConfig, got {other:?}"),
        }
    }

    #[test]
    fn no_targets_anywhere_is_flagged() {
        let mut p = baseline_profile();
        p.output_groups[0].stream_targets.clear();
        match FFmpegHandler::validate_config(&p) {
            Err(CoreError::InvalidStreamConfig { reasons }) => {
                assert!(reasons.iter().any(|r| r.code == "no_stream_targets"));
            }
            other => panic!("expected InvalidStreamConfig, got {other:?}"),
        }
    }

    #[test]
    fn empty_target_url_is_flagged() {
        let mut p = baseline_profile();
        p.output_groups[0].stream_targets[0].url = "".into();
        match FFmpegHandler::validate_config(&p) {
            Err(CoreError::InvalidStreamConfig { reasons }) => {
                assert!(reasons.iter().any(|r| r.code == "target_missing_url"));
            }
            other => panic!("expected InvalidStreamConfig, got {other:?}"),
        }
    }

    #[test]
    fn empty_stream_key_is_flagged() {
        let mut p = baseline_profile();
        p.output_groups[0].stream_targets[0].stream_key = "".into();
        match FFmpegHandler::validate_config(&p) {
            Err(CoreError::InvalidStreamConfig { reasons }) => {
                assert!(reasons
                    .iter()
                    .any(|r| r.code == "target_missing_stream_key"));
            }
            other => panic!("expected InvalidStreamConfig, got {other:?}"),
        }
    }

    #[test]
    fn non_passthrough_with_odd_dimensions_is_flagged() {
        let mut p = baseline_profile();
        p.output_groups[0].video = VideoSettings {
            codec: "libx264".into(),
            width: 1281,
            height: 720,
            fps: 30,
            bitrate: "6000k".into(),
            preset: None,
            profile: None,
            keyframe_interval_seconds: Some(2),
        };
        p.output_groups[0].audio = AudioSettings {
            codec: "aac".into(),
            bitrate: "160k".into(),
            channels: 2,
            sample_rate: 48000,
        };
        match FFmpegHandler::validate_config(&p) {
            Err(CoreError::InvalidStreamConfig { reasons }) => {
                assert!(reasons.iter().any(|r| r.code == "video_resolution_odd"));
            }
            other => panic!("expected InvalidStreamConfig, got {other:?}"),
        }
    }

    #[test]
    fn non_passthrough_with_bitrate_above_max_is_flagged() {
        let mut p = baseline_profile();
        p.output_groups[0].video = VideoSettings {
            codec: "libx264".into(),
            width: 1920,
            height: 1080,
            fps: 30,
            bitrate: "60000k".into(),
            preset: None,
            profile: None,
            keyframe_interval_seconds: Some(2),
        };
        p.output_groups[0].audio = AudioSettings {
            codec: "aac".into(),
            bitrate: "160k".into(),
            channels: 2,
            sample_rate: 48000,
        };
        let err = FFmpegHandler::validate_config(&p).unwrap_err();
        match err {
            CoreError::InvalidStreamConfig { reasons } => {
                assert!(reasons
                    .iter()
                    .any(|r| r.code == "video_bitrate_out_of_range"));
            }
            other => panic!("expected InvalidStreamConfig, got {other:?}"),
        }
    }

    #[test]
    fn non_passthrough_with_fps_above_max_is_flagged() {
        let mut p = baseline_profile();
        p.output_groups[0].video = VideoSettings {
            codec: "libx264".into(),
            width: 1920,
            height: 1080,
            fps: 300,
            bitrate: "6000k".into(),
            preset: None,
            profile: None,
            keyframe_interval_seconds: Some(2),
        };
        p.output_groups[0].audio = AudioSettings {
            codec: "aac".into(),
            bitrate: "160k".into(),
            channels: 2,
            sample_rate: 48000,
        };
        match FFmpegHandler::validate_config(&p) {
            Err(CoreError::InvalidStreamConfig { reasons }) => {
                assert!(reasons.iter().any(|r| r.code == "video_fps_out_of_range"));
            }
            other => panic!("expected InvalidStreamConfig, got {other:?}"),
        }
    }

    #[test]
    fn non_passthrough_with_keyframe_above_max_is_flagged() {
        let mut p = baseline_profile();
        p.output_groups[0].video = VideoSettings {
            codec: "libx264".into(),
            width: 1920,
            height: 1080,
            fps: 30,
            bitrate: "6000k".into(),
            preset: None,
            profile: None,
            keyframe_interval_seconds: Some(60),
        };
        p.output_groups[0].audio = AudioSettings {
            codec: "aac".into(),
            bitrate: "160k".into(),
            channels: 2,
            sample_rate: 48000,
        };
        match FFmpegHandler::validate_config(&p) {
            Err(CoreError::InvalidStreamConfig { reasons }) => {
                assert!(reasons
                    .iter()
                    .any(|r| r.code == "video_keyframe_interval_out_of_range"));
            }
            other => panic!("expected InvalidStreamConfig, got {other:?}"),
        }
    }

    #[test]
    fn parse_bitrate_handles_k_m_and_bare() {
        assert_eq!(parse_bitrate_to_kbps("6000k"), Some(6000));
        assert_eq!(parse_bitrate_to_kbps("6000K"), Some(6000));
        assert_eq!(parse_bitrate_to_kbps("8M"), Some(8000));
        assert_eq!(parse_bitrate_to_kbps("8m"), Some(8000));
        assert_eq!(parse_bitrate_to_kbps("6000000"), Some(6000));
        assert_eq!(parse_bitrate_to_kbps(""), None);
        assert_eq!(parse_bitrate_to_kbps("nope"), None);
    }

    /// FLV container only carries H.264 video. H.265/AV1/VP9 inside FLV
    /// produces a malformed stream that RTMP receivers reject mid-handshake;
    /// catch it at validation time before any FFmpeg process spawns.
    ///
    #[test]
    fn flv_with_hevc_video_is_flagged() {
        let mut p = baseline_profile();
        p.output_groups[0].video = VideoSettings {
            codec: "libx265".into(),
            width: 1280,
            height: 720,
            fps: 30,
            bitrate: "6000k".into(),
            preset: None,
            profile: None,
            keyframe_interval_seconds: Some(2),
        };
        p.output_groups[0].audio = AudioSettings {
            codec: "aac".into(),
            bitrate: "128k".into(),
            channels: 2,
            sample_rate: 48000,
        };
        p.output_groups[0].container = ContainerSettings {
            format: "flv".into(),
        };
        match FFmpegHandler::validate_config(&p) {
            Err(CoreError::InvalidStreamConfig { reasons }) => {
                assert!(
                    reasons
                        .iter()
                        .any(|r| r.code == "video_codec_container_incompatible"),
                    "expected video_codec_container_incompatible, got {reasons:?}"
                );
            }
            other => panic!("expected InvalidStreamConfig, got {other:?}"),
        }
    }

    #[test]
    fn flv_with_opus_audio_is_flagged() {
        let mut p = baseline_profile();
        p.output_groups[0].video = VideoSettings {
            codec: "libx264".into(),
            width: 1280,
            height: 720,
            fps: 30,
            bitrate: "6000k".into(),
            preset: None,
            profile: None,
            keyframe_interval_seconds: Some(2),
        };
        p.output_groups[0].audio = AudioSettings {
            codec: "libopus".into(),
            bitrate: "128k".into(),
            channels: 2,
            sample_rate: 48000,
        };
        p.output_groups[0].container = ContainerSettings {
            format: "flv".into(),
        };
        match FFmpegHandler::validate_config(&p) {
            Err(CoreError::InvalidStreamConfig { reasons }) => {
                assert!(
                    reasons
                        .iter()
                        .any(|r| r.code == "audio_codec_container_incompatible"),
                    "expected audio_codec_container_incompatible, got {reasons:?}"
                );
            }
            other => panic!("expected InvalidStreamConfig, got {other:?}"),
        }
    }

    /// H.264 + AAC inside FLV is the canonical streaming combo; must pass.
    #[test]
    fn flv_with_h264_aac_validates() {
        let mut p = baseline_profile();
        p.output_groups[0].video = VideoSettings {
            codec: "libx264".into(),
            width: 1280,
            height: 720,
            fps: 30,
            bitrate: "6000k".into(),
            preset: None,
            profile: None,
            keyframe_interval_seconds: Some(2),
        };
        p.output_groups[0].audio = AudioSettings {
            codec: "aac".into(),
            bitrate: "128k".into(),
            channels: 2,
            sample_rate: 48000,
        };
        p.output_groups[0].container = ContainerSettings {
            format: "flv".into(),
        };
        FFmpegHandler::validate_config(&p).expect("h264/aac/flv must pass");
    }
}
