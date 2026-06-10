use std::collections::HashSet;

use crate::errors::CoreError;
use crate::models::OutputGroup;
use crate::services::PlatformRegistry;

use super::ffmpeg_internal;

/// Stream-URL schemes that may carry a credential and must be redacted before
/// a command line or ffmpeg log line is written. RTMP(S) keys live in the path;
/// HTTP(S) ingests may carry the key in the path or query string.
const STREAM_URL_SCHEMES: [&str; 4] = ["rtmp://", "rtmps://", "https://", "http://"];

/// True if `arg` contains any redactable stream-URL scheme.
fn contains_stream_url(arg: &str) -> bool {
    STREAM_URL_SCHEMES.iter().any(|scheme| arg.contains(scheme))
}

/// Byte offset of the earliest stream-URL scheme in `segment`, if any.
fn find_stream_url_start(segment: &str) -> Option<usize> {
    STREAM_URL_SCHEMES
        .iter()
        .filter_map(|scheme| segment.find(scheme))
        .min()
}

impl super::FFmpegHandler {
    /// Normalize an RTMP URL for consistency.
    pub(super) fn normalize_rtmp_url(url: &str) -> String {
        let mut url = url.trim().to_string();

        while url.ends_with('/') {
            url.pop();
        }

        if !url.starts_with("rtmp://") && !url.starts_with("rtmps://") {
            // rtmps for known port / host hints, rtmp otherwise.
            if url.contains(":443") || url.contains("facebook.com") {
                url = format!("rtmps://{url}");
            } else {
                url = format!("rtmp://{url}");
            }
        }

        url
    }

    /// Sanitize a single argument with platform context for accurate redaction.
    pub(super) fn sanitize_arg_with_context(&self, arg: &str, group: &OutputGroup) -> String {
        if !contains_stream_url(arg) {
            return arg.to_string();
        }

        let mut parts = Vec::new();
        for segment in arg.split('|') {
            let redacted = if let Some(pos) = find_stream_url_start(segment) {
                let prefix = &segment[..pos];
                let url_start = pos;
                let url_end = segment[url_start..]
                    .find(' ')
                    .map(|i| url_start + i)
                    .unwrap_or(segment.len());
                let url = &segment[url_start..url_end];
                let suffix = &segment[url_end..];

                let platform_redacted = group
                    .stream_targets
                    .iter()
                    .find(|target| {
                        let normalized = Self::normalize_rtmp_url(&target.url);
                        url.starts_with(&normalized) || url.contains(&target.url)
                    })
                    .and_then(|target| {
                        self.platform_registry.get(&target.service).map(|config| {
                            format!("[{}] {}", config.display_name(), config.redact_url(url))
                        })
                    });

                let redacted_url =
                    platform_redacted.unwrap_or_else(|| PlatformRegistry::generic_redact(url));

                format!("{prefix}{redacted_url}{suffix}")
            } else {
                segment.to_string()
            };
            parts.push(redacted);
        }

        parts.join("|")
    }

    /// Sanitize all FFmpeg arguments (redact stream keys) with platform-aware redaction.
    pub(super) fn sanitize_ffmpeg_args(&self, args: &[String], group: &OutputGroup) -> Vec<String> {
        args.iter()
            .map(|arg| self.sanitize_arg_with_context(arg, group))
            .collect()
    }

    /// Static version of sanitize_arg for use in background threads.
    /// Uses generic platform-agnostic redaction.
    pub(super) fn sanitize_arg_static(arg: &str) -> String {
        if !contains_stream_url(arg) {
            return arg.to_string();
        }

        let mut parts = Vec::new();
        for segment in arg.split('|') {
            let redacted = if let Some(pos) = find_stream_url_start(segment) {
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

    /// Resolve stream key — supports `${ENV_VAR}` syntax.
    pub(super) fn resolve_stream_key(key: &str) -> String {
        if key.starts_with("${") && key.ends_with("}") && key.len() > 3 {
            let var_name = &key[2..key.len() - 1];
            match std::env::var(var_name) {
                Ok(value) => {
                    // Security: never log the variable name (would reveal which
                    // env vars carry credentials).
                    log::debug!("Resolved stream key from environment variable");
                    value
                }
                Err(_) => {
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

    /// Build FFmpeg arguments for the shared relay process.
    pub(super) fn build_relay_args(
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

    pub(super) fn double_bitrate_value(bitrate: &str) -> Option<String> {
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

    pub(super) fn append_cbr_args(args: &mut Vec<String>, encoder: &str, bitrate: &str) {
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

    pub(super) fn map_nvenc_preset(preset: &str) -> String {
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

    /// Add RTMP protocol options to a URL for connection resilience.
    ///
    /// Matches OBS Studio's RTMP configuration for maximum stability:
    /// - timeout: 30 seconds (matching OBS receive timeout)
    /// - rtmp_buffer: 30 seconds (matching OBS buffer, 10x default)
    /// - tcp_keepalive: enabled to detect dead connections via TCP probes
    /// - rtmp_live: live stream mode (optimizes for live rather than VOD)
    pub(super) fn add_rtmp_options(url: &str) -> String {
        if !url.starts_with("rtmp://") && !url.starts_with("rtmps://") {
            return url.to_string();
        }

        // FFmpeg RTMP protocol options verified in FFmpeg documentation.
        // https://ffmpeg.org/ffmpeg-protocols.html#rtmp
        let options = [
            ("timeout", "30000000"),  // 30s receive timeout (microseconds)
            ("rtmp_buffer", "30000"), // 30s client buffer (milliseconds)
            ("tcp_keepalive", "1"),   // Enable TCP keepalive probes
            ("rtmp_live", "live"),    // Live stream mode
        ];

        let separator = if url.contains('?') { "&" } else { "?" };

        let query = options
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");

        format!("{url}{separator}{query}")
    }

    /// Build FFmpeg arguments for an output group.
    ///
    /// Groups read from the shared TCP relay so they can restart independently.
    pub(super) fn build_args(&self, group: &OutputGroup) -> Vec<String> {
        // Stream-copy passthrough when both video and audio codecs are "copy" —
        // FFmpeg acts as a pure RTMP relay (no re-encoding). Case-insensitive
        // so "Copy"/"COPY" match.
        let use_stream_copy = group.video.codec.eq_ignore_ascii_case("copy")
            && group.audio.codec.eq_ignore_ascii_case("copy");

        let mut args = Vec::new();

        // Build fflags: always discard corrupt frames, optionally generate PTS.
        // +discardcorrupt: silently drop corrupt frames (always good for live).
        // +genpts: generate fresh presentation timestamps (helps timing issues).
        let fflags = if group.generate_pts {
            "+discardcorrupt+genpts"
        } else {
            "+discardcorrupt"
        };
        args.push("-fflags".to_string());
        args.push(fflags.to_string());

        if group.generate_pts {
            args.push("-copyts".to_string());
        }

        args.push("-i".to_string());
        args.push(self.relay_input_url_for_group(&group.id));

        // Audio sync when PTS generation is enabled.
        // -async 1: resample audio to match timestamps, fixing drift.
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
            args.push("-c:v".to_string());
            args.push(group.video.codec.clone());
            args.push("-s".to_string());
            args.push(group.video.resolution());
            args.push("-b:v".to_string());
            args.push(group.video.bitrate.clone());
            Self::append_cbr_args(&mut args, &group.video.codec, &group.video.bitrate);
            args.push("-r".to_string());
            args.push(group.video.fps.to_string());
            args.push("-c:a".to_string());
            args.push(group.audio.codec.clone());
            args.push("-b:a".to_string());
            args.push(group.audio.bitrate.clone());
            args.push("-ac".to_string());
            args.push(group.audio.channels.to_string());
            args.push("-ar".to_string());
            args.push(group.audio.sample_rate.to_string());
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

        args.push("-map".to_string());
        args.push("0:v".to_string());
        args.push("-map".to_string());
        args.push("0:a".to_string());

        args.push("-progress".to_string());
        args.push("pipe:2".to_string());
        args.push("-stats".to_string());

        let disabled = self.disabled_targets.lock().unwrap_or_else(|e| {
            log::warn!("Disabled targets mutex poisoned (build_args), recovering: {e}");
            e.into_inner()
        });
        let mut target_outputs: Vec<String> = Vec::new();
        for target in &group.stream_targets {
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

        // Always use onfail=ignore for RTMP outputs to prevent one failed
        // connection from killing the stats meter and potentially other outputs.
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
