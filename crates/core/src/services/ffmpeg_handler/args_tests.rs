//! Unit tests for the FFmpeg argument builders in `args.rs`.
//!
//! These exercise the deterministic, side-effect-free surface: URL
//! normalization, stream-key redaction (a safety-critical path — a leak
//! here exposes a creator's stream key in logs), CBR / preset / keyframe
//! argument construction, and the full `build_args` / `build_relay_args`
//! assembly. No FFmpeg process is ever spawned; the handler is built with
//! a deliberately missing binary path so construction stays hermetic.

use super::FFmpegHandler;
use crate::models::Platform;
use crate::models::{AudioSettings, ContainerSettings, OutputGroup, StreamTarget, VideoSettings};
use std::collections::HashSet;

fn handler() -> FFmpegHandler {
    // A non-existent path makes `ffmpeg_path` resolve to the missing
    // sentinel deterministically, with no `$PATH` / env probe.
    FFmpegHandler::new_with_custom_path(
        std::path::PathBuf::from("/tmp"),
        Some("/nonexistent/spiritstream-test-ffmpeg".into()),
    )
    .expect("handler constructs without a real ffmpeg binary")
}

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

fn target(id: &str, url: &str, key: &str) -> StreamTarget {
    StreamTarget {
        id: id.into(),
        name: id.into(),
        service: Platform::Custom,
        url: url.into(),
        stream_key: key.into(),
    }
}

fn group_with(
    video: VideoSettings,
    audio: AudioSettings,
    targets: Vec<StreamTarget>,
) -> OutputGroup {
    OutputGroup {
        id: "g1".into(),
        name: "Group 1".into(),
        is_default: true,
        generate_pts: false,
        video,
        audio,
        container: ContainerSettings::default(),
        stream_targets: targets,
    }
}

/// Find the argument immediately following `flag` (FFmpeg's flag/value
/// pairing) for structural assertions on the built arg vector.
fn value_after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
}

// ---------------------------------------------------------------------------
// normalize_rtmp_url
// ---------------------------------------------------------------------------

#[test]
fn normalize_trims_trailing_slashes_and_whitespace() {
    assert_eq!(
        FFmpegHandler::normalize_rtmp_url("  rtmp://host/app///  "),
        "rtmp://host/app"
    );
}

#[test]
fn normalize_adds_rtmp_scheme_when_missing() {
    assert_eq!(
        FFmpegHandler::normalize_rtmp_url("host/app"),
        "rtmp://host/app"
    );
}

#[test]
fn normalize_prefers_rtmps_for_port_443_and_facebook() {
    assert_eq!(
        FFmpegHandler::normalize_rtmp_url("edge.example:443/live"),
        "rtmps://edge.example:443/live"
    );
    assert_eq!(
        FFmpegHandler::normalize_rtmp_url("live-api-s.facebook.com/rtmp"),
        "rtmps://live-api-s.facebook.com/rtmp"
    );
}

#[test]
fn normalize_leaves_already_schemed_urls_untouched() {
    assert_eq!(
        FFmpegHandler::normalize_rtmp_url("rtmps://secure/live"),
        "rtmps://secure/live"
    );
}

// ---------------------------------------------------------------------------
// double_bitrate_value
// ---------------------------------------------------------------------------

#[test]
fn double_bitrate_handles_suffixes_and_bare_numbers() {
    assert_eq!(
        FFmpegHandler::double_bitrate_value("6000k"),
        Some("12000k".into())
    );
    assert_eq!(
        FFmpegHandler::double_bitrate_value("8M"),
        Some("16M".into())
    );
    assert_eq!(
        FFmpegHandler::double_bitrate_value("4.5M"),
        Some("9M".into())
    );
    assert_eq!(
        FFmpegHandler::double_bitrate_value("6000"),
        Some("12000".into())
    );
}

#[test]
fn double_bitrate_rejects_empty_and_non_numeric() {
    assert_eq!(FFmpegHandler::double_bitrate_value(""), None);
    assert_eq!(FFmpegHandler::double_bitrate_value("   "), None);
    assert_eq!(FFmpegHandler::double_bitrate_value("nope"), None);
}

// ---------------------------------------------------------------------------
// map_nvenc_preset
// ---------------------------------------------------------------------------

#[test]
fn nvenc_preset_passes_through_native_pn_values() {
    assert_eq!(FFmpegHandler::map_nvenc_preset("p4"), "p4");
    assert_eq!(FFmpegHandler::map_nvenc_preset("lossless"), "lossless");
}

#[test]
fn nvenc_preset_maps_x264_style_names() {
    assert_eq!(FFmpegHandler::map_nvenc_preset("ultrafast"), "p1");
    assert_eq!(FFmpegHandler::map_nvenc_preset("veryslow"), "p7");
    assert_eq!(FFmpegHandler::map_nvenc_preset("low-latency"), "p1");
}

#[test]
fn nvenc_preset_defaults_to_p4_for_empty_or_unknown() {
    assert_eq!(FFmpegHandler::map_nvenc_preset(""), "p4");
    assert_eq!(FFmpegHandler::map_nvenc_preset("bogus"), "p4");
    // Case-insensitive normalization.
    assert_eq!(FFmpegHandler::map_nvenc_preset("FAST"), "fast");
}

// ---------------------------------------------------------------------------
// add_rtmp_options
// ---------------------------------------------------------------------------

#[test]
fn add_rtmp_options_appends_resilience_query() {
    let out = FFmpegHandler::add_rtmp_options("rtmp://host/app");
    assert!(out.starts_with("rtmp://host/app?"));
    assert!(out.contains("timeout=30000000"));
    assert!(out.contains("rtmp_buffer=30000"));
    assert!(out.contains("tcp_keepalive=1"));
    assert!(out.contains("rtmp_live=live"));
}

#[test]
fn add_rtmp_options_uses_ampersand_when_query_present() {
    let out = FFmpegHandler::add_rtmp_options("rtmp://host/app?foo=1");
    assert!(out.starts_with("rtmp://host/app?foo=1&timeout="));
}

#[test]
fn add_rtmp_options_passes_non_rtmp_through() {
    assert_eq!(
        FFmpegHandler::add_rtmp_options("http://host/x"),
        "http://host/x"
    );
}

// ---------------------------------------------------------------------------
// sanitize_arg_static / sanitize_ffmpeg_args
// ---------------------------------------------------------------------------

#[test]
fn sanitize_static_redacts_stream_key_in_rtmp_url() {
    let redacted =
        FFmpegHandler::sanitize_arg_static("rtmp://custom-server.com/stream/my_secret_key");
    assert_eq!(redacted, "rtmp://custom-server.com/stream/***");
    assert!(!redacted.contains("my_secret_key"));
}

#[test]
fn sanitize_static_leaves_non_url_args_untouched() {
    assert_eq!(FFmpegHandler::sanitize_arg_static("-c:v"), "-c:v");
    assert_eq!(FFmpegHandler::sanitize_arg_static("libx264"), "libx264");
}

#[test]
fn sanitize_static_redacts_stream_key_in_https_url() {
    // A custom HTTPS ingest can carry the key in the path or query string;
    // the static sanitizer must mask both before the command line is logged.
    let redacted =
        FFmpegHandler::sanitize_arg_static("https://ingest.example.com/live/my_secret_key");
    assert_eq!(redacted, "https://ingest.example.com/live/***");
    assert!(!redacted.contains("my_secret_key"));

    let query = FFmpegHandler::sanitize_arg_static("https://host/upload?cid=my_secret_key&copy=0");
    assert!(
        !query.contains("my_secret_key"),
        "query key leaked: {query}"
    );
}

#[test]
fn sanitize_ffmpeg_args_redacts_each_url_arg() {
    let h = handler();
    let group = group_with(
        copy_video(),
        copy_audio(),
        vec![target(
            "t1",
            "rtmp://custom-server.com/stream",
            "supersecret",
        )],
    );
    let args = vec![
        "-i".to_string(),
        "rtmp://custom-server.com/stream/supersecret".to_string(),
    ];
    let sanitized = h.sanitize_ffmpeg_args(&args, &group);
    assert!(sanitized.iter().all(|a| !a.contains("supersecret")));
}

// ---------------------------------------------------------------------------
// resolve_stream_key
// ---------------------------------------------------------------------------

#[test]
fn resolve_stream_key_expands_env_reference() {
    std::env::set_var("SPIRITSTREAM_TEST_KEY_RESOLVE", "resolved-value");
    assert_eq!(
        FFmpegHandler::resolve_stream_key("${SPIRITSTREAM_TEST_KEY_RESOLVE}"),
        "resolved-value"
    );
    std::env::remove_var("SPIRITSTREAM_TEST_KEY_RESOLVE");
}

#[test]
fn resolve_stream_key_keeps_literal_when_env_missing() {
    assert_eq!(
        FFmpegHandler::resolve_stream_key("${SPIRITSTREAM_TEST_KEY_DEFINITELY_ABSENT}"),
        "${SPIRITSTREAM_TEST_KEY_DEFINITELY_ABSENT}"
    );
}

#[test]
fn resolve_stream_key_passes_plain_keys_through() {
    assert_eq!(FFmpegHandler::resolve_stream_key("plain-key"), "plain-key");
}

// ---------------------------------------------------------------------------
// append_cbr_args
// ---------------------------------------------------------------------------

#[test]
fn append_cbr_sets_min_max_buf_and_doubles_bufsize() {
    let mut args = Vec::new();
    FFmpegHandler::append_cbr_args(&mut args, "libx264", "6000k");
    assert_eq!(value_after(&args, "-minrate"), Some("6000k"));
    assert_eq!(value_after(&args, "-maxrate"), Some("6000k"));
    assert_eq!(value_after(&args, "-bufsize"), Some("12000k"));
    // libx264 gets the HRD CBR force-cfr params.
    assert_eq!(
        value_after(&args, "-x264-params"),
        Some("nal-hrd=cbr:force-cfr=1")
    );
}

#[test]
fn append_cbr_adds_rc_cbr_for_hardware_encoders() {
    let mut args = Vec::new();
    FFmpegHandler::append_cbr_args(&mut args, "h264_nvenc", "8000k");
    assert_eq!(value_after(&args, "-rc"), Some("cbr"));
}

#[test]
fn append_cbr_uses_x265_params_for_hevc() {
    let mut args = Vec::new();
    FFmpegHandler::append_cbr_args(&mut args, "libx265", "5000k");
    assert_eq!(value_after(&args, "-x265-params"), Some("nal-hrd=cbr"));
}

// ---------------------------------------------------------------------------
// build_relay_args
// ---------------------------------------------------------------------------

#[test]
fn build_relay_args_errors_on_empty_group_set() {
    let h = handler();
    let empty: HashSet<String> = HashSet::new();
    assert!(h.build_relay_args("rtmp://in/live", &empty).is_err());
}

#[test]
fn build_relay_args_produces_listen_copy_tee_pipeline() {
    let h = handler();
    let mut ids = HashSet::new();
    ids.insert("g1".to_string());
    let args = h
        .build_relay_args("rtmp://in/live", &ids)
        .expect("relay args build for a non-empty group set");
    assert_eq!(value_after(&args, "-listen"), Some("1"));
    assert_eq!(value_after(&args, "-c:v"), Some("copy"));
    assert_eq!(value_after(&args, "-c:a"), Some("copy"));
    assert_eq!(value_after(&args, "-f"), Some("tee"));
}

// ---------------------------------------------------------------------------
// build_args — passthrough vs re-encode
// ---------------------------------------------------------------------------

#[test]
fn build_args_passthrough_copies_codecs() {
    let h = handler();
    let group = group_with(
        copy_video(),
        copy_audio(),
        vec![target("t1", "rtmp://custom/live", "key1")],
    );
    let args = h.build_args(&group);
    assert_eq!(value_after(&args, "-c:v"), Some("copy"));
    assert_eq!(value_after(&args, "-c:a"), Some("copy"));
    // A target is present, so the tee muxer is emitted.
    assert_eq!(value_after(&args, "-f"), Some("tee"));
    // The stats meter output is always teed in alongside real targets.
    let tee = args.last().expect("tee output string");
    assert!(tee.contains("onfail=ignore"));
}

#[test]
fn build_args_reencode_emits_video_and_audio_settings() {
    let h = handler();
    let video = VideoSettings {
        codec: "libx264".into(),
        width: 1280,
        height: 720,
        fps: 30,
        bitrate: "6000k".into(),
        preset: Some("quality".into()),
        profile: Some("high".into()),
        keyframe_interval_seconds: Some(2),
    };
    let audio = AudioSettings {
        codec: "aac".into(),
        bitrate: "160k".into(),
        channels: 2,
        sample_rate: 48000,
    };
    let group = group_with(
        video,
        audio,
        vec![target("t1", "rtmp://custom/live", "key1")],
    );
    let args = h.build_args(&group);

    assert_eq!(value_after(&args, "-c:v"), Some("libx264"));
    assert_eq!(value_after(&args, "-s"), Some("1280x720"));
    assert_eq!(value_after(&args, "-b:v"), Some("6000k"));
    assert_eq!(value_after(&args, "-r"), Some("30"));
    assert_eq!(value_after(&args, "-c:a"), Some("aac"));
    assert_eq!(value_after(&args, "-b:a"), Some("160k"));
    assert_eq!(value_after(&args, "-profile:v"), Some("high"));
    // libx264 "quality" preset maps to "slow".
    assert_eq!(value_after(&args, "-preset"), Some("slow"));
    // keyframe interval 2s @ 30fps → GOP 60.
    assert_eq!(value_after(&args, "-g"), Some("60"));
    assert_eq!(value_after(&args, "-keyint_min"), Some("60"));
    assert_eq!(
        value_after(&args, "-force_key_frames"),
        Some("expr:gte(t,n_forced*2)")
    );
}

#[test]
fn build_args_amf_low_latency_preset_emits_quality_and_usage() {
    let h = handler();
    let video = VideoSettings {
        codec: "h264_amf".into(),
        width: 1920,
        height: 1080,
        fps: 60,
        bitrate: "8000k".into(),
        preset: Some("low_latency".into()),
        profile: None,
        keyframe_interval_seconds: None,
    };
    let audio = AudioSettings {
        codec: "aac".into(),
        bitrate: "160k".into(),
        channels: 2,
        sample_rate: 48000,
    };
    let group = group_with(
        video,
        audio,
        vec![target("t1", "rtmp://custom/live", "key1")],
    );
    let args = h.build_args(&group);
    // AMF maps low-latency to "-quality speed -usage lowlatency".
    assert_eq!(value_after(&args, "-quality"), Some("speed"));
    assert_eq!(value_after(&args, "-usage"), Some("lowlatency"));
    // AMF must not emit the x264-style "-preset" flag.
    assert!(value_after(&args, "-preset").is_none());
}

#[test]
fn build_args_nvenc_quality_preset_maps_to_p7() {
    let h = handler();
    let video = VideoSettings {
        codec: "hevc_nvenc".into(),
        width: 1280,
        height: 720,
        fps: 30,
        bitrate: "6000k".into(),
        preset: Some("quality".into()),
        profile: None,
        keyframe_interval_seconds: None,
    };
    let audio = AudioSettings {
        codec: "aac".into(),
        bitrate: "128k".into(),
        channels: 2,
        sample_rate: 44100,
    };
    let group = group_with(
        video,
        audio,
        vec![target("t1", "rtmp://custom/live", "key1")],
    );
    let args = h.build_args(&group);
    // NVENC "quality" maps to the slowest p-state, p7.
    assert_eq!(value_after(&args, "-preset"), Some("p7"));
    // NVENC must not emit the AMF-style "-quality" flag.
    assert!(value_after(&args, "-quality").is_none());
}

#[test]
fn build_args_libx264_low_latency_preset_maps_to_ultrafast() {
    let h = handler();
    let video = VideoSettings {
        codec: "libx264".into(),
        width: 1280,
        height: 720,
        fps: 30,
        bitrate: "4500k".into(),
        preset: Some("low_latency".into()),
        profile: None,
        keyframe_interval_seconds: None,
    };
    let audio = AudioSettings {
        codec: "aac".into(),
        bitrate: "128k".into(),
        channels: 2,
        sample_rate: 48000,
    };
    let group = group_with(
        video,
        audio,
        vec![target("t1", "rtmp://custom/live", "key1")],
    );
    let args = h.build_args(&group);
    assert_eq!(value_after(&args, "-preset"), Some("ultrafast"));
}

#[test]
fn build_args_flv_container_forces_codec_tags() {
    let h = handler();
    let mut group = group_with(
        copy_video(),
        copy_audio(),
        vec![target("t1", "rtmp://custom/live", "key1")],
    );
    group.container = ContainerSettings {
        format: "flv".into(),
    };
    let args = h.build_args(&group);
    assert_eq!(value_after(&args, "-tag:v"), Some("7"));
    assert_eq!(value_after(&args, "-tag:a"), Some("10"));
    // Stream-copy into FLV needs the ADTS→ASC bitstream filter.
    assert_eq!(value_after(&args, "-bsf:a"), Some("aac_adtstoasc"));
}

#[test]
fn build_args_generate_pts_adds_copyts_and_async() {
    let h = handler();
    let mut group = group_with(
        copy_video(),
        copy_audio(),
        vec![target("t1", "rtmp://custom/live", "key1")],
    );
    group.generate_pts = true;
    let args = h.build_args(&group);
    assert!(args.iter().any(|a| a == "-copyts"));
    assert_eq!(
        value_after(&args, "-fflags"),
        Some("+discardcorrupt+genpts")
    );
    assert_eq!(value_after(&args, "-async"), Some("1"));
}

#[test]
fn build_args_skips_disabled_targets_and_omits_tee_when_all_disabled() {
    let h = handler();
    let group = group_with(
        copy_video(),
        copy_audio(),
        vec![target("only", "rtmp://custom/live", "key1")],
    );
    h.disable_target("only");
    let args = h.build_args(&group);
    // The single target is disabled → no real outputs → the early return
    // fires before the tee muxer is appended.
    assert!(!args.iter().any(|a| a == "tee"));
}
