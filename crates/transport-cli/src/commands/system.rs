//! `spiritstream-cli system …` — environmental capability probes.

use std::collections::BTreeMap;

use clap::Subcommand;
use serde::Serialize;
use spiritstream_core::ServiceRegistry;

use crate::error::CliError;
use crate::output::Output;

#[derive(Debug, Subcommand)]
pub enum SystemCmd {
    /// List FFmpeg-detected video and audio encoders.
    Encoders,
    /// FFmpeg-specific operations (check installation, download, cancel).
    Ffmpeg {
        #[command(subcommand)]
        action: FfmpegAction,
    },
    /// Print server-tuned client constants (polling intervals, popup
    /// dimensions, per-platform chat char limits, …).
    ClientConfig,
    /// Print the encoder preset matrix (resolution/fps/bitrate options,
    /// per-codec presets, container formats).
    EncoderPresets,
    /// Print recent server log lines (default 500). The frontend Logs view
    /// reads via this surface.
    Logs {
        #[arg(long, default_value_t = 500)]
        lines: usize,
    },
    /// Test an RTMP target by attempting a quick connection.
    TestRtmp {
        url: String,
        /// Source for the stream key — stdin pipe or interactive
        /// prompt. Stream keys never ride argv (`ps` / shell history).
        #[arg(long = "stream-key-from", value_enum)]
        stream_key_from: crate::secret_input::SecretSource,
    },
    /// Validate a path points at an FFmpeg binary, return its version string.
    ValidateFfmpegPath { path: std::path::PathBuf },
    /// Print the running app version. Mirrors `GET /api/v1/system/app-version`.
    AppVersion,
    /// Export the current log file to `<out>`. Path must lie inside the
    /// data directory or the user's home. Mirrors
    /// `POST /api/v1/system/logs/export` — the HTTP route takes the
    /// content from the client; the CLI reads it locally.
    LogsExport {
        #[arg(long)]
        out: std::path::PathBuf,
        /// Override the line cap (default: full file).
        #[arg(long)]
        lines: Option<usize>,
    },
    /// Record an app-update failure into the HMAC-chained audit log.
    /// Mirrors `POST /api/v1/system/audit/app-update-failure`.
    AuditUpdateFailure {
        /// Sanitized error message from the updater.
        #[arg(long)]
        detail: String,
    },
    /// Aggregate subsystem health (profiles, settings, themes, audit-log
    /// chain). Mirrors `GET /api/v1/health`. Use `--subsystem <name>` to
    /// narrow to a single subsystem and exit with non-zero on
    /// non-`ok` status.
    Health {
        #[arg(long)]
        subsystem: Option<String>,
    },
    /// Readiness probe — same checks as `health` but reports a single
    /// boolean and exits non-zero when not ready. Mirrors
    /// `GET /api/v1/ready`. The CLI builds the registry synchronously so
    /// "ready" reduces to "every health subsystem reports `Ok`".
    Ready,
}

#[derive(Debug, Subcommand)]
pub enum FfmpegAction {
    /// Probe whether FFmpeg is reachable and report its version.
    Check,
    /// Print the resolved FFmpeg path or `null` if discovery fails.
    Path,
    /// Check whether a newer FFmpeg version is available upstream.
    UpdateCheck {
        #[arg(long)]
        installed_version: Option<String>,
    },
}

#[derive(Serialize)]
struct FfmpegProbeOk {
    available: bool,
    version: String,
}

#[derive(Serialize)]
struct FfmpegProbeMissing {
    available: bool,
    detail: String,
}

pub async fn run(
    cmd: SystemCmd,
    registry: &ServiceRegistry,
    out: &mut Output,
) -> Result<(), CliError> {
    match cmd {
        SystemCmd::Encoders => {
            let encoders = spiritstream_core::commands::get_encoders()?;
            out.emit(&encoders)?;
            Ok(())
        }
        SystemCmd::Ffmpeg {
            action: FfmpegAction::Check,
        } => match spiritstream_core::commands::test_ffmpeg() {
            Ok(version) => out.emit(&FfmpegProbeOk {
                available: true,
                version,
            }),
            Err(err) => out.emit(&FfmpegProbeMissing {
                available: false,
                detail: err.to_string(),
            }),
        },
        SystemCmd::Ffmpeg {
            action: FfmpegAction::Path,
        } => {
            use spiritstream_core::services::FFmpegLocator;
            let path = FFmpegLocator::discover(Some(&registry.settings));
            out.emit(&serde_json::json!({
                "path": path.map(|p| p.to_string_lossy().to_string()),
            }))?;
            Ok(())
        }
        SystemCmd::Ffmpeg {
            action: FfmpegAction::UpdateCheck { installed_version },
        } => {
            let info = registry
                .ffmpeg_locator
                .check_version_status(installed_version.as_deref())
                .await;
            out.emit(&info)?;
            Ok(())
        }
        SystemCmd::ClientConfig => {
            // Mirror the HTTP /api/v1/system/client-config payload exactly.
            use spiritstream_core::models::ChatPlatform;
            let mut chat_max_chars = std::collections::HashMap::new();
            for p in [
                ChatPlatform::Twitch,
                ChatPlatform::YouTube,
                ChatPlatform::Trovo,
                ChatPlatform::Kick,
                ChatPlatform::Facebook,
                ChatPlatform::TikTok,
            ] {
                chat_max_chars.insert(p.as_str().to_owned(), p.max_message_chars() as u32);
            }
            out.emit(&serde_json::json!({
                "obsTriggerDelayMs": 2000_u32,
                "autoSaveDelayMs": 500_u32,
                "chatPollIntervalMs": 5000_u32,
                "toastDurationMs": 4000_u32,
                "chatPopupWidth": 420_u32,
                "chatPopupHeight": 720_u32,
                "chatOverlayPollMs": 500_u32,
                "retryBaseDelayMs": 800_u32,
                "chatMaxChars": chat_max_chars,
            }))?;
            Ok(())
        }
        SystemCmd::EncoderPresets => {
            // Static preset matrix shared with HTTP /api/v1/system/encoders/presets.
            let mut presets = std::collections::HashMap::new();
            presets.insert(
                "libx264".to_string(),
                vec![
                    "ultrafast",
                    "superfast",
                    "veryfast",
                    "faster",
                    "fast",
                    "medium",
                    "slow",
                    "slower",
                    "veryslow",
                ]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>(),
            );
            presets.insert(
                "libx265".to_string(),
                vec![
                    "ultrafast",
                    "superfast",
                    "veryfast",
                    "faster",
                    "fast",
                    "medium",
                    "slow",
                    "slower",
                    "veryslow",
                ]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>(),
            );
            presets.insert(
                "nvenc".to_string(),
                vec!["p1", "p2", "p3", "p4", "p5", "p6", "p7"]
                    .into_iter()
                    .map(String::from)
                    .collect::<Vec<_>>(),
            );
            presets.insert(
                "amf".to_string(),
                vec!["quality", "balanced", "speed"]
                    .into_iter()
                    .map(String::from)
                    .collect::<Vec<_>>(),
            );
            out.emit(&serde_json::json!({
                "resolutions": ["1920x1080","1280x720","2560x1440","3840x2160","854x480"],
                "fpsValues": ["60","30","24","25","50"],
                "audioBitrates": ["320k","256k","192k","160k","128k","96k","64k"],
                "audioChannels": ["1","2","6","8"],
                "audioSampleRates": ["48000","44100","32000"],
                "containerFormats": ["flv","mpegts","mp4"],
                "h264Profiles": ["baseline","main","high"],
                "presets": presets,
            }))?;
            Ok(())
        }
        SystemCmd::Logs { lines } => {
            let logs = spiritstream_core::services::read_recent_logs(&registry.log_dir, lines)?;
            out.emit(&logs)?;
            Ok(())
        }
        SystemCmd::TestRtmp {
            url,
            stream_key_from,
        } => {
            let stream_key =
                crate::secret_input::read_secret(stream_key_from, "Stream key")?;
            let result =
                spiritstream_core::commands::test_rtmp_target(url, stream_key.to_string())?;
            out.emit(&result)?;
            Ok(())
        }
        SystemCmd::ValidateFfmpegPath { path } => {
            let p = path.to_string_lossy().to_string();
            let validated = spiritstream_core::commands::validate_ffmpeg_path(p)?;
            out.emit(&serde_json::json!({ "version": validated }))?;
            Ok(())
        }
        SystemCmd::AppVersion => {
            // Same source the HTTP `/system/app-version` route returns —
            // the CLI binary carries its own `CARGO_PKG_VERSION` but the
            // release workflow validates every version string matches.
            out.emit(&serde_json::json!({
                "version": env!("CARGO_PKG_VERSION"),
            }))?;
            Ok(())
        }
        SystemCmd::LogsExport {
            out: out_path,
            lines,
        } => {
            // Documented contract: destinations inside the data dir OR
            // the user's home are allowed. The code used to allow only
            // the data dir, silently contradicting its own docs.
            let home = dirs_next::home_dir();
            let mut allowed: Vec<&std::path::Path> = vec![registry.data_dir.as_path()];
            if let Some(home) = home.as_deref() {
                allowed.push(home);
            }
            let validated =
                spiritstream_core::services::validate_path_within_any(&out_path, &allowed)?;
            let cap = lines.unwrap_or(usize::MAX);
            let log_lines = spiritstream_core::services::read_recent_logs(&registry.log_dir, cap)?;
            let body = log_lines.join("\n");
            std::fs::write(&validated, &body)
                .map_err(|e| CliError::Io(format!("write {}: {}", validated.display(), e)))?;
            out.emit(&serde_json::json!({
                "exported": true,
                "path": validated.to_string_lossy(),
                "lines": log_lines.len(),
            }))?;
            Ok(())
        }
        SystemCmd::AuditUpdateFailure { detail } => {
            registry.audit.record(
                spiritstream_core::services::AuditAction::AppUpdateSignatureFailed { detail },
            )?;
            out.emit(&serde_json::json!({ "recorded": true }))?;
            Ok(())
        }
        SystemCmd::Health { subsystem } => {
            let report = compute_health(registry).await;
            match subsystem {
                Some(name) => match report.services.get(&name) {
                    None => Err(CliError::Argument(format!(
                        "unknown subsystem: {name} (valid: {})",
                        report
                            .services
                            .keys()
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))),
                    Some(status) => {
                        let entry = serde_json::json!({
                            "status": &report.status,
                            "subsystem": &name,
                            "report": status,
                        });
                        out.emit(&entry)?;
                        if !matches!(status, SubsystemStatus::Ok) {
                            // A degraded service is an availability
                            // condition (EX_UNAVAILABLE, 69), not a
                            // caller-usage error — retry-loop scripts
                            // branch on the exit code.
                            return Err(CliError::Unavailable(format!(
                                "subsystem '{name}' is not ok"
                            )));
                        }
                        Ok(())
                    }
                },
                None => {
                    out.emit(&report)?;
                    if report.status != "ok" {
                        return Err(CliError::Unavailable(format!(
                            "system health: {}",
                            report.status
                        )));
                    }
                    Ok(())
                }
            }
        }
        SystemCmd::Ready => {
            let report = compute_health(registry).await;
            let ready = report.status == "ok";
            let failed: Vec<String> = report
                .services
                .iter()
                .filter(|(_, v)| !matches!(v, SubsystemStatus::Ok))
                .map(|(k, _)| k.clone())
                .collect();
            out.emit(&serde_json::json!({
                "ready": ready,
                "failed": failed,
            }))?;
            if !ready {
                // EX_UNAVAILABLE (69) so retry-loop scripts can
                // distinguish "service starting up" from "bad flags."
                return Err(CliError::Unavailable("not ready".into()));
            }
            Ok(())
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case", tag = "state")]
enum SubsystemStatus {
    Ok,
    Degraded { detail: String },
    Disconnected { detail: String },
    Tampered { last_valid_sequence: u64 },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthReport {
    /// Aggregate status — `"ok"`, `"degraded"`, or `"tampered"`. Same
    /// rollup rule the HTTP `/health` route uses.
    status: String,
    services: BTreeMap<String, SubsystemStatus>,
}

async fn compute_health(registry: &ServiceRegistry) -> HealthReport {
    let mut services: BTreeMap<String, SubsystemStatus> = BTreeMap::new();
    services.insert(
        "profiles".into(),
        match registry.profiles.get_all_names().await {
            Ok(_) => SubsystemStatus::Ok,
            Err(err) => SubsystemStatus::Disconnected {
                detail: err.to_string(),
            },
        },
    );
    services.insert(
        "settings".into(),
        match registry.settings.load() {
            Ok(_) => SubsystemStatus::Ok,
            Err(err) => SubsystemStatus::Disconnected {
                detail: err.to_string(),
            },
        },
    );
    services.insert(
        "themes".into(),
        if registry.themes.list_themes().is_empty() {
            SubsystemStatus::Degraded {
                detail: "no themes loaded".into(),
            }
        } else {
            SubsystemStatus::Ok
        },
    );
    services.insert(
        "audit_log".into(),
        match registry.audit.verify_chain() {
            Ok(spiritstream_core::services::AuditChainStatus::Ok { .. }) => SubsystemStatus::Ok,
            Ok(spiritstream_core::services::AuditChainStatus::Empty) => SubsystemStatus::Ok,
            Ok(spiritstream_core::services::AuditChainStatus::Tampered {
                last_valid_sequence,
                ..
            }) => SubsystemStatus::Tampered {
                last_valid_sequence,
            },
            Err(e) => SubsystemStatus::Disconnected {
                detail: e.to_string(),
            },
        },
    );

    let status = if services
        .values()
        .any(|s| matches!(s, SubsystemStatus::Tampered { .. }))
    {
        "tampered"
    } else if services.values().any(|s| {
        matches!(
            s,
            SubsystemStatus::Disconnected { .. } | SubsystemStatus::Degraded { .. }
        )
    }) {
        "degraded"
    } else {
        "ok"
    };

    HealthReport {
        status: status.into(),
        services,
    }
}
