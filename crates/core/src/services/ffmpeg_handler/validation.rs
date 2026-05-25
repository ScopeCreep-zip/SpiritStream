use std::collections::VecDeque;

use crate::errors::{CoreError, ValidationIssue};
use crate::models::{OutputGroup, Profile};

/// Encoding-config bounds the plan calls out for `StreamService::validate_config`.
/// Out-of-range values surface as `CoreError::InvalidStreamConfig` so the
/// frontend can highlight specific fields rather than parsing message strings.
pub const VIDEO_BITRATE_MIN_KBPS: u32 = 500;
pub const VIDEO_BITRATE_MAX_KBPS: u32 = 50_000;
pub const KEYFRAME_INTERVAL_MIN_SECS: u32 = 1;
pub const KEYFRAME_INTERVAL_MAX_SECS: u32 = 10;
pub const FPS_MIN: u32 = 1;
pub const FPS_MAX: u32 = 240;

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

impl super::FFmpegHandler {
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
        let issues = Self::collect_group_issues(group, "/group");
        if issues.is_empty() {
            Ok(())
        } else {
            Err(CoreError::InvalidStreamConfig { reasons: issues })
        }
    }

    /// Extracted per-group rule set so both `validate_config` (full profile)
    /// and `validate_output_group` (single group) share the same matrix.
    pub(super) fn collect_group_issues(group: &OutputGroup, base: &str) -> Vec<ValidationIssue> {
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

    /// Parse Windows error codes and FFmpeg error codes from log lines.
    /// Called from `stats_reader` to enrich the `stream_error` payload
    /// when an FFmpeg process exits unexpectedly.
    pub(super) fn parse_error_details(lines: &VecDeque<String>) -> Option<String> {
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
}
