// Shared FFmpeg Argument Builder
// Extracts duplicated FFmpeg argument construction patterns used across
// ffmpeg_handler, recording_service, and replay_buffer.

/// Builder for constructing FFmpeg command-line arguments.
///
/// Provides reusable methods for common FFmpeg argument patterns that are
/// shared across streaming, recording, and replay buffer services.
pub struct FfmpegArgsBuilder {
    args: Vec<String>,
}

impl FfmpegArgsBuilder {
    /// Create a new empty argument builder.
    pub fn new() -> Self {
        Self { args: Vec::new() }
    }

    /// Add raw video input from stdin (`pipe:0`).
    ///
    /// Generates: `-f rawvideo -pix_fmt <fmt> -s <W>x<H> -r <fps> -i pipe:0`
    pub fn rawvideo_input(
        &mut self,
        width: u32,
        height: u32,
        fps: u32,
        pixel_format: &str,
    ) -> &mut Self {
        self.args.extend([
            "-f".to_string(),
            "rawvideo".to_string(),
            "-pix_fmt".to_string(),
            pixel_format.to_string(),
            "-s".to_string(),
            format!("{}x{}", width, height),
            "-r".to_string(),
            fps.to_string(),
            "-i".to_string(),
            "pipe:0".to_string(),
        ]);
        self
    }

    /// Add video encoder arguments: codec, bitrate, and CBR enforcement.
    ///
    /// Applies CBR rate control flags appropriate for the specified encoder.
    pub fn video_encoder(&mut self, codec: &str, bitrate: &str) -> &mut Self {
        self.args.extend([
            "-c:v".to_string(),
            codec.to_string(),
            "-b:v".to_string(),
            bitrate.to_string(),
        ]);
        append_cbr_args(&mut self.args, codec, bitrate);
        self
    }

    /// Add video encoder preset, mapped to the correct flag for the encoder.
    ///
    /// Handles NVENC (`-preset p1..p7`), AMF (`-quality`, `-usage`),
    /// and software (`-preset ultrafast..veryslow`) presets.
    pub fn video_preset(&mut self, codec: &str, preset: &str) -> &mut Self {
        if codec.contains("amf") {
            let mut amf_quality: Option<&str> = None;
            let mut amf_usage: Option<&str> = None;
            match preset {
                "quality" => amf_quality = Some("quality"),
                "balanced" => amf_quality = Some("balanced"),
                "speed" => amf_quality = Some("speed"),
                "performance" | "fast" | "faster" | "veryfast" | "superfast" | "ultrafast" => {
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
                self.args.push("-quality".to_string());
                self.args.push(quality.to_string());
            }
            if let Some(usage) = amf_usage {
                self.args.push("-usage".to_string());
                self.args.push(usage.to_string());
            }
        } else if codec.contains("nvenc") {
            let ffmpeg_preset = map_nvenc_preset(preset);
            self.args.push("-preset".to_string());
            self.args.push(ffmpeg_preset);
        } else {
            let supports_preset = codec == "libx264" || codec == "libx265";
            if supports_preset {
                let ffmpeg_preset = match preset {
                    "quality" => "slow",
                    "balanced" => "medium",
                    "performance" => "fast",
                    "low_latency" | "low-latency" | "lowLatency" => "ultrafast",
                    other => other,
                };
                self.args.push("-preset".to_string());
                self.args.push(ffmpeg_preset.to_string());
            }
        }
        self
    }

    /// Add H.264/H.265 profile constraint (`-profile:v`).
    pub fn video_profile(&mut self, profile: &str) -> &mut Self {
        self.args.push("-profile:v".to_string());
        self.args.push(profile.to_string());
        self
    }

    /// Add keyframe interval arguments for the given encoder.
    ///
    /// Sets `-g`, `-keyint_min`, `-sc_threshold`, and `-force_key_frames` as appropriate.
    pub fn keyframe_interval(
        &mut self,
        codec: &str,
        fps: u32,
        interval_seconds: u32,
    ) -> &mut Self {
        if interval_seconds == 0 || fps == 0 {
            return self;
        }
        let gop_size = fps.saturating_mul(interval_seconds);
        if gop_size == 0 {
            return self;
        }

        self.args.push("-g".to_string());
        self.args.push(gop_size.to_string());

        if codec == "libx264" || codec == "libx265" {
            self.args.push("-keyint_min".to_string());
            self.args.push(gop_size.to_string());
            self.args.push("-sc_threshold".to_string());
            self.args.push("0".to_string());
        }

        self.args.push("-force_key_frames".to_string());
        self.args.push(format!("expr:gte(t,n_forced*{interval_seconds})"));
        self
    }

    /// Add audio encoder arguments: codec, bitrate, channels, and sample rate.
    pub fn audio_encoder(
        &mut self,
        codec: &str,
        bitrate: &str,
        channels: u32,
        sample_rate: u32,
    ) -> &mut Self {
        self.args.extend([
            "-c:a".to_string(),
            codec.to_string(),
            "-b:a".to_string(),
            bitrate.to_string(),
            "-ac".to_string(),
            channels.to_string(),
            "-ar".to_string(),
            sample_rate.to_string(),
        ]);
        self
    }

    /// Add output resolution and frame rate.
    pub fn video_scale(&mut self, resolution: &str, fps: u32) -> &mut Self {
        self.args.push("-s".to_string());
        self.args.push(resolution.to_string());
        self.args.push("-r".to_string());
        self.args.push(fps.to_string());
        self
    }

    /// Add stream copy for both video and audio (`-c:v copy -c:a copy`).
    pub fn stream_copy(&mut self) -> &mut Self {
        self.args.extend([
            "-c:v".to_string(),
            "copy".to_string(),
            "-c:a".to_string(),
            "copy".to_string(),
        ]);
        self
    }

    /// Push arbitrary arguments.
    pub fn push(&mut self, args: &[&str]) -> &mut Self {
        for arg in args {
            self.args.push(arg.to_string());
        }
        self
    }

    /// Consume the builder and return the argument list.
    pub fn build(self) -> Vec<String> {
        self.args
    }


    /// Get a reference to the current arguments.
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// Get a mutable reference to the underlying argument vector.
    pub fn args_mut(&mut self) -> &mut Vec<String> {
        &mut self.args
    }
}

impl Default for FfmpegArgsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Shared helper functions (extracted from FFmpegHandler)
// ============================================================================

/// Append CBR (constant bitrate) enforcement flags for the given encoder.
///
/// Sets `-minrate`, `-maxrate`, `-bufsize`, and encoder-specific rate control flags.
pub fn append_cbr_args(args: &mut Vec<String>, encoder: &str, bitrate: &str) {
    let bufsize = double_bitrate_value(bitrate)
        .unwrap_or_else(|| bitrate.to_string());

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

/// Map a user-facing preset name to an NVENC-compatible preset string.
///
/// Accepts both NVENC-native names (`p1`..`p7`) and common aliases
/// (`ultrafast`, `quality`, `balanced`, etc.).
pub fn map_nvenc_preset(preset: &str) -> String {
    let normalized = preset.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return "p4".to_string();
    }

    match normalized.as_str() {
        "p1" | "p2" | "p3" | "p4" | "p5" | "p6" | "p7" | "default" | "slow" | "medium"
        | "fast" | "hp" | "hq" | "bd" | "ll" | "llhq" | "llhp" | "lossless"
        | "losslesshp" => normalized,
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

/// Double a bitrate value string (e.g. `"4000k"` → `"8000k"`), preserving any suffix.
///
/// Returns `None` if the input is empty or not parseable.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rawvideo_input() {
        let mut builder = FfmpegArgsBuilder::new();
        builder.rawvideo_input(1920, 1080, 30, "bgra");
        let args = builder.build();
        assert_eq!(args, vec![
            "-f", "rawvideo", "-pix_fmt", "bgra",
            "-s", "1920x1080", "-r", "30", "-i", "pipe:0",
        ]);
    }

    #[test]
    fn test_video_encoder_x264() {
        let mut builder = FfmpegArgsBuilder::new();
        builder.video_encoder("libx264", "4000k");
        let args = builder.build();
        assert!(args.contains(&"-c:v".to_string()));
        assert!(args.contains(&"libx264".to_string()));
        assert!(args.contains(&"-b:v".to_string()));
        assert!(args.contains(&"4000k".to_string()));
        // CBR args
        assert!(args.contains(&"-minrate".to_string()));
        assert!(args.contains(&"-maxrate".to_string()));
        assert!(args.contains(&"-x264-params".to_string()));
    }

    #[test]
    fn test_stream_copy() {
        let mut builder = FfmpegArgsBuilder::new();
        builder.stream_copy();
        let args = builder.build();
        assert_eq!(args, vec!["-c:v", "copy", "-c:a", "copy"]);
    }

    #[test]
    fn test_map_nvenc_preset_aliases() {
        assert_eq!(map_nvenc_preset("ultrafast"), "p1");
        assert_eq!(map_nvenc_preset("quality"), "p7");
        assert_eq!(map_nvenc_preset("balanced"), "p4");
        assert_eq!(map_nvenc_preset(""), "p4");
        assert_eq!(map_nvenc_preset("p3"), "p3");
    }

    #[test]
    fn test_double_bitrate_value() {
        assert_eq!(double_bitrate_value("4000k"), Some("8000k".to_string()));
        assert_eq!(double_bitrate_value("2.5M"), Some("5M".to_string()));
        assert_eq!(double_bitrate_value(""), None);
    }

    #[test]
    fn test_video_preset_amf() {
        let mut builder = FfmpegArgsBuilder::new();
        builder.video_preset("h264_amf", "low_latency");
        let args = builder.build();
        assert!(args.contains(&"-quality".to_string()));
        assert!(args.contains(&"speed".to_string()));
        assert!(args.contains(&"-usage".to_string()));
        assert!(args.contains(&"lowlatency".to_string()));
    }

    #[test]
    fn test_keyframe_interval() {
        let mut builder = FfmpegArgsBuilder::new();
        builder.keyframe_interval("libx264", 30, 2);
        let args = builder.build();
        assert!(args.contains(&"-g".to_string()));
        assert!(args.contains(&"60".to_string()));
        assert!(args.contains(&"-keyint_min".to_string()));
        assert!(args.contains(&"-sc_threshold".to_string()));
        assert!(args.contains(&"-force_key_frames".to_string()));
    }
}
