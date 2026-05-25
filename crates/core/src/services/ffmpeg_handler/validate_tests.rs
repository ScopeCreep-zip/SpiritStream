use super::{parse_bitrate_to_kbps, FFmpegHandler};
use crate::errors::CoreError;
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
