// FFmpeg Argument Building
// All FFmpeg command-line argument construction logic for output groups and native capture

use crate::models::OutputGroup;

use super::ffmpeg_relay::FFmpegRelay;
use super::ffmpeg_stats;
use super::FFmpegHandler;

impl FFmpegHandler {
    /// Build FFmpeg arguments for an output group
    ///
    /// Groups read from the shared TCP relay so they can restart independently.
    pub(super) fn build_args(&self, group: &OutputGroup) -> Vec<String> {
        // Determine if we should use stream copy (passthrough mode)
        // When both video and audio codecs are set to "copy", FFmpeg acts as a pure
        // RTMP relay server, accepting the incoming stream and forwarding it to outputs
        // without re-encoding. This is the default behavior and most efficient mode.
        // Use case-insensitive comparison to handle "Copy", "COPY", etc.
        let use_stream_copy = group.video.codec.eq_ignore_ascii_case("copy")
            && group.audio.codec.eq_ignore_ascii_case("copy");

        let mut args = vec![
            "-i".to_string(),
            FFmpegRelay::relay_input_url_for_group(&group.id),
        ];

        if use_stream_copy {
            args.push("-c:v".to_string()); args.push("copy".to_string());
            args.push("-c:a".to_string()); args.push("copy".to_string());
        } else {
            use crate::services::ffmpeg_args::FfmpegArgsBuilder;

            let mut builder = FfmpegArgsBuilder::new();
            // Video settings
            builder.video_encoder(&group.video.codec, &group.video.bitrate);
            builder.video_scale(&group.video.resolution(), group.video.fps);
            // Audio settings
            builder.audio_encoder(
                &group.audio.codec,
                &group.audio.bitrate,
                group.audio.channels.into(),
                group.audio.sample_rate,
            );
            // Video encoder preset if specified
            if let Some(preset) = &group.video.preset {
                builder.video_preset(&group.video.codec, preset);
            }
            // H.264 profile if specified
            if let Some(profile) = &group.video.profile {
                builder.video_profile(profile);
            }
            // Keyframe interval
            if let Some(interval_seconds) = group.video.keyframe_interval_seconds {
                builder.keyframe_interval(&group.video.codec, group.video.fps, interval_seconds);
            }

            args.extend(builder.build());
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
        args.push("-map".to_string()); args.push("0:v".to_string());
        args.push("-map".to_string()); args.push("0:a".to_string());

        // Progress output for stats parsing
        args.push("-progress".to_string()); args.push("pipe:2".to_string());
        args.push("-stats".to_string());

        // Add output targets (skip disabled ones)
        let mut target_outputs: Vec<String> = Vec::new();
        for target in &group.stream_targets {
            // Skip targets that have been disabled via toggle_target
            if self.disabled_targets.contains(&target.id) {
                continue;
            }

            let normalized_url = Self::normalize_rtmp_url(&target.url);
            let normalized_url = self.platform_registry.normalize_url(&target.service, &normalized_url);
            let resolved_key = Self::resolve_stream_key(&target.stream_key);
            let full_url = self.platform_registry.build_url_with_key(&target.service, &normalized_url, &resolved_key);

            // Log the platform we're streaming to
            if let Some(config) = self.platform_registry.get(&target.service) {
                log::debug!("Adding output target: {} ({})", config.display_name(), target.id);
            }

            target_outputs.push(full_url);
        }

        if target_outputs.is_empty() {
            return args;
        }

        let meter_output = ffmpeg_stats::meter_output_url_for_group(&group.id);

        let mut tee_outputs: Vec<String> = Vec::new();
        if target_outputs.len() == 1 {
            let output = &target_outputs[0];
            tee_outputs.push(format!("[f={}]{output}", group.container.format));
        } else {
            tee_outputs.extend(
                target_outputs
                    .iter()
                    .map(|output| format!("[f={}:onfail=ignore]{output}", group.container.format))
            );
        }
        tee_outputs.push(format!("[f=mpegts:onfail=ignore]{meter_output}"));

        args.push("-f".to_string());
        args.push("tee".to_string());
        args.push(tee_outputs.join("|"));

        args
    }

    /// Build FFmpeg arguments for native video input
    pub(super) fn build_native_video_args(&self, group: &OutputGroup, video_config: &super::native::NativeVideoConfig) -> Vec<String> {
        use crate::services::ffmpeg_args::FfmpegArgsBuilder;

        let mut builder = FfmpegArgsBuilder::new();
        builder.rawvideo_input(
            video_config.width,
            video_config.height,
            video_config.fps,
            &video_config.pixel_format,
        );

        builder.video_encoder(&group.video.codec, &group.video.bitrate);

        if let Some(preset) = &group.video.preset {
            builder.video_preset(&group.video.codec, preset);
        }
        if let Some(profile) = &group.video.profile {
            builder.video_profile(profile);
        }

        let mut args = builder.build();

        // No audio for video-only
        args.push("-an".to_string());

        // Output targets
        self.append_output_targets(&mut args, group);

        args
    }

    /// Build FFmpeg arguments for native audio input
    pub(super) fn build_native_audio_args(&self, group: &OutputGroup, audio_config: &super::native::NativeAudioConfig) -> Vec<String> {
        // Raw audio input has a unique format (sample_format as -f flag), not suitable for builder
        let mut args = vec![
            "-f".to_string(), audio_config.sample_format.clone(),
            "-ar".to_string(), audio_config.sample_rate.to_string(),
            "-ac".to_string(), audio_config.channels.to_string(),
            "-i".to_string(), "pipe:0".to_string(),
        ];

        // Audio encoding
        args.push("-c:a".to_string());
        args.push(group.audio.codec.clone());
        args.push("-b:a".to_string());
        args.push(group.audio.bitrate.clone());

        // No video for audio-only
        args.push("-vn".to_string());

        // Output targets
        self.append_output_targets(&mut args, group);

        args
    }

    /// Build FFmpeg arguments for combined native A/V input
    ///
    /// Uses nut muxer to interleave video and audio in a single stdin pipe.
    /// Caller must mux video and audio frames into nut format before writing.
    pub(super) fn build_native_av_args(
        &self,
        group: &OutputGroup,
        video_config: &super::native::NativeVideoConfig,
        audio_config: &super::native::NativeAudioConfig,
    ) -> Vec<String> {
        use crate::services::ffmpeg_args::FfmpegArgsBuilder;

        // For combined A/V, we use a simple approach: read interleaved raw data
        // The caller is responsible for properly interleaving video and audio frames
        let mut builder = FfmpegArgsBuilder::new();
        builder.rawvideo_input(
            video_config.width,
            video_config.height,
            video_config.fps,
            &video_config.pixel_format,
        );

        // For now, we only support video-only through the single stdin
        // Full A/V would require named pipes or a more complex approach

        // Video encoding
        builder.video_encoder(&group.video.codec, &group.video.bitrate);

        if let Some(preset) = &group.video.preset {
            builder.video_preset(&group.video.codec, preset);
        }
        if let Some(profile) = &group.video.profile {
            builder.video_profile(profile);
        }

        // Audio encoding (will need separate audio input in future)
        builder.audio_encoder(
            &group.audio.codec,
            &group.audio.bitrate,
            audio_config.channels as u32,
            audio_config.sample_rate,
        );

        let mut args = builder.build();

        // Output targets
        self.append_output_targets(&mut args, group);

        args
    }

    /// Helper to append output targets to FFmpeg args
    pub(super) fn append_output_targets(&self, args: &mut Vec<String>, group: &OutputGroup) {
        // Progress output for stats parsing
        args.push("-progress".to_string());
        args.push("pipe:2".to_string());
        args.push("-stats".to_string());

        // Check disabled targets
        let mut target_outputs: Vec<String> = Vec::new();
        for target in &group.stream_targets {
            if self.disabled_targets.contains(&target.id) {
                continue;
            }

            let normalized_url = Self::normalize_rtmp_url(&target.url);
            let normalized_url = self.platform_registry.normalize_url(&target.service, &normalized_url);
            let resolved_key = Self::resolve_stream_key(&target.stream_key);
            let full_url = self.platform_registry.build_url_with_key(&target.service, &normalized_url, &resolved_key);

            // Log the platform we're streaming to
            if let Some(config) = self.platform_registry.get(&target.service) {
                log::debug!("Adding output target: {} ({})", config.display_name(), target.id);
            }

            target_outputs.push(full_url);
        }

        if target_outputs.is_empty() {
            return;
        }

        let meter_output = ffmpeg_stats::meter_output_url_for_group(&group.id);

        let mut tee_outputs: Vec<String> = Vec::new();
        if target_outputs.len() == 1 {
            let output = &target_outputs[0];
            tee_outputs.push(format!("[f={}]{output}", group.container.format));
        } else {
            tee_outputs.extend(
                target_outputs
                    .iter()
                    .map(|output| format!("[f={}:onfail=ignore]{output}", group.container.format))
            );
        }
        tee_outputs.push(format!("[f=mpegts:onfail=ignore]{meter_output}"));

        args.push("-f".to_string());
        args.push("tee".to_string());
        args.push(tee_outputs.join("|"));
    }
}
