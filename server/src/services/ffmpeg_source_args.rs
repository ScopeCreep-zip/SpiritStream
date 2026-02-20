// Shared FFmpeg Source Input Args Builder
// Single source of truth for converting Source → FFmpeg input arguments.
// Used by both preview_handler (single-source preview) and compositor (scene compositing).

use crate::models::Source;

/// Options controlling how FFmpeg input args are generated.
/// Preview and compositor modes differ in validation, pixel format, and placeholder sizing.
pub struct SourceArgsOptions {
    /// Add `-pixel_format uyvy422` on macOS for Camera/Screen/CaptureCard
    pub include_pixel_format: bool,
    /// Add `-rtmp_live live` and convert 0.0.0.0 → 127.0.0.1 for RTMP sources
    pub include_rtmp_live: bool,
    /// Use `:none` suffix for video-only AVFoundation (no audio) on screen/camera/card
    pub audio_none_suffix: bool,
    /// Width/height for lavfi placeholder sources (Color, Text, Browser, etc.)
    pub placeholder_width: u32,
    pub placeholder_height: u32,
    /// Validate device IDs and return errors for empty values
    pub validate_device_ids: bool,
}

/// Preview mode options: strict validation, pixel format, 640x360 placeholders
pub const PREVIEW_OPTS: SourceArgsOptions = SourceArgsOptions {
    include_pixel_format: true,
    include_rtmp_live: true,
    audio_none_suffix: true,
    placeholder_width: 640,
    placeholder_height: 360,
    validate_device_ids: true,
};

/// Compositor mode options: permissive, no pixel format, 1920x1080 placeholders
pub const COMPOSITOR_OPTS: SourceArgsOptions = SourceArgsOptions {
    include_pixel_format: false,
    include_rtmp_live: false,
    audio_none_suffix: false,
    placeholder_width: 1920,
    placeholder_height: 1080,
    validate_device_ids: false,
};

/// Build FFmpeg input arguments for a source.
///
/// Returns `Ok(args)` on success, `Err(message)` if validation fails (only when
/// `opts.validate_device_ids` is true).
pub fn source_input_args(source: &Source, opts: &SourceArgsOptions) -> Result<Vec<String>, String> {
    match source {
        Source::Camera(cam) => {
            if opts.validate_device_ids && cam.device_id.is_empty() {
                return Err("Camera device not selected".to_string());
            }
            Ok(camera_args(&cam.device_id, cam.width, cam.height, cam.fps, opts))
        }

        Source::ScreenCapture(screen) => {
            if opts.validate_device_ids && screen.display_id.is_empty() {
                return Err("Display not selected".to_string());
            }
            Ok(screen_capture_args(
                &screen.display_id,
                screen.fps,
                screen.capture_cursor,
                screen.capture_audio,
                opts,
            ))
        }

        Source::MediaFile(media) => {
            if opts.validate_device_ids && media.file_path.is_empty() {
                return Err("Media file path not specified".to_string());
            }
            Ok(media_file_args(&media.file_path, media.loop_playback))
        }

        Source::CaptureCard(card) => {
            if opts.validate_device_ids && card.device_id.is_empty() {
                return Err("Capture card device not selected".to_string());
            }
            Ok(capture_card_args(&card.device_id, opts))
        }

        Source::Rtmp(rtmp) => Ok(rtmp_args(
            &rtmp.bind_address,
            rtmp.port,
            &rtmp.application,
            opts,
        )),

        Source::AudioDevice(audio) => {
            if opts.audio_none_suffix {
                // Preview mode: show placeholder visual for audio-only source
                Ok(placeholder_args("darkblue", opts))
            } else {
                // Compositor mode: actual audio device input
                Ok(audio_device_args(&audio.device_id))
            }
        }

        Source::Color(color) => {
            let hex = color.color.trim_start_matches('#');
            Ok(vec![
                "-f".to_string(),
                "lavfi".to_string(),
                "-i".to_string(),
                format!(
                    "color=c=0x{}:s={}x{}:r=30:d=3600",
                    hex, opts.placeholder_width, opts.placeholder_height
                ),
            ])
        }

        Source::Text(_) => {
            let color_str = if opts.audio_none_suffix {
                "black" // preview
            } else {
                "black@0" // compositor (transparent)
            };
            Ok(vec![
                "-f".to_string(),
                "lavfi".to_string(),
                "-i".to_string(),
                format!(
                    "color=c={}:s={}x{}:r=30:d=3600",
                    color_str, opts.placeholder_width, opts.placeholder_height
                ),
            ])
        }

        Source::Browser(_) => Ok(placeholder_args("gray", opts)),

        Source::WindowCapture(win) => {
            if opts.validate_device_ids && win.window_id.is_empty() {
                return Err("Window not selected".to_string());
            }
            Ok(window_capture_args(win, opts))
        }

        Source::MediaPlaylist(_) => Ok(placeholder_args("0x00008B", opts)),
        Source::NestedScene(_) => Ok(placeholder_args("0x800080", opts)),
        Source::GameCapture(_) => Ok(placeholder_args("0x006400", opts)),
        Source::Ndi(_) => Ok(placeholder_args("0xFF8C00", opts)),
    }
}

// ============================================================================
// Per-source arg builders
// ============================================================================

fn camera_args(
    device_id: &str,
    width: Option<u32>,
    height: Option<u32>,
    fps: Option<u32>,
    opts: &SourceArgsOptions,
) -> Vec<String> {
    let mut args = Vec::new();

    #[cfg(target_os = "macos")]
    {
        args.extend(["-f".to_string(), "avfoundation".to_string()]);
        if let Some(fps) = fps {
            args.extend(["-framerate".to_string(), fps.to_string()]);
        } else {
            args.extend(["-framerate".to_string(), "30".to_string()]);
        }
        if opts.include_pixel_format {
            args.extend(["-pixel_format".to_string(), "uyvy422".to_string()]);
        }
        if let (Some(w), Some(h)) = (width, height) {
            args.extend(["-video_size".to_string(), format!("{}x{}", w, h)]);
        }
        let device_input = if device_id.contains(':') {
            device_id.to_string()
        } else if opts.audio_none_suffix {
            format!("{}:none", device_id)
        } else {
            format!("{}:", device_id)
        };
        args.extend(["-i".to_string(), device_input]);
    }

    #[cfg(target_os = "windows")]
    {
        args.extend(["-f".to_string(), "dshow".to_string()]);
        if let Some(fps) = fps {
            args.extend(["-framerate".to_string(), fps.to_string()]);
        }
        if let (Some(w), Some(h)) = (width, height) {
            args.extend(["-video_size".to_string(), format!("{}x{}", w, h)]);
        }
        args.extend(["-i".to_string(), format!("video={}", device_id)]);
    }

    #[cfg(target_os = "linux")]
    {
        args.extend(["-f".to_string(), "v4l2".to_string()]);
        if let Some(fps) = fps {
            args.extend(["-framerate".to_string(), fps.to_string()]);
        }
        if let (Some(w), Some(h)) = (width, height) {
            args.extend(["-video_size".to_string(), format!("{}x{}", w, h)]);
        }
        args.extend(["-i".to_string(), device_id.to_string()]);
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    { let _ = (device_id, width, height, fps, opts); }

    args
}

fn screen_capture_args(
    display_id: &str,
    fps: u32,
    capture_cursor: bool,
    capture_audio: bool,
    opts: &SourceArgsOptions,
) -> Vec<String> {
    let mut args = Vec::new();

    #[cfg(target_os = "macos")]
    {
        args.extend(["-f".to_string(), "avfoundation".to_string()]);
        args.extend(["-framerate".to_string(), fps.to_string()]);
        args.extend([
            "-capture_cursor".to_string(),
            if capture_cursor { "1" } else { "0" }.to_string(),
        ]);
        if opts.include_pixel_format {
            args.extend(["-pixel_format".to_string(), "uyvy422".to_string()]);
        }
        let screen_input = if opts.audio_none_suffix && !capture_audio {
            format!("{}:none", display_id)
        } else {
            format!("{}:", display_id)
        };
        args.extend(["-i".to_string(), screen_input]);
    }

    #[cfg(target_os = "windows")]
    {
        args.extend(["-f".to_string(), "gdigrab".to_string()]);
        args.extend(["-framerate".to_string(), fps.to_string()]);
        if capture_cursor {
            args.extend(["-draw_mouse".to_string(), "1".to_string()]);
        }
        args.extend(["-i".to_string(), "desktop".to_string()]);
    }

    #[cfg(target_os = "linux")]
    {
        args.extend(["-f".to_string(), "x11grab".to_string()]);
        args.extend(["-framerate".to_string(), fps.to_string()]);
        if capture_cursor {
            args.extend(["-draw_mouse".to_string(), "1".to_string()]);
        }
        args.extend(["-i".to_string(), format!(":{}", display_id)]);
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    { let _ = (display_id, fps, capture_cursor, capture_audio, opts); }

    args
}

fn media_file_args(file_path: &str, loop_playback: bool) -> Vec<String> {
    let mut args = Vec::new();
    if loop_playback {
        args.extend(["-stream_loop".to_string(), "-1".to_string()]);
    } else {
        args.extend(["-stream_loop".to_string(), "0".to_string()]);
    }
    args.extend(["-i".to_string(), file_path.to_string()]);
    args
}

fn capture_card_args(device_id: &str, opts: &SourceArgsOptions) -> Vec<String> {
    let mut args = Vec::new();

    #[cfg(target_os = "macos")]
    {
        args.extend(["-f".to_string(), "avfoundation".to_string()]);
        if opts.include_pixel_format {
            args.extend(["-pixel_format".to_string(), "uyvy422".to_string()]);
        }
        let device_input = if device_id.contains(':') {
            device_id.to_string()
        } else if opts.audio_none_suffix {
            format!("{}:none", device_id)
        } else {
            format!("{}:", device_id)
        };
        args.extend(["-i".to_string(), device_input]);
    }

    #[cfg(target_os = "windows")]
    {
        args.extend(["-f".to_string(), "dshow".to_string()]);
        if opts.audio_none_suffix {
            args.extend([
                "-i".to_string(),
                format!("video={}:audio={}", device_id, device_id),
            ]);
        } else {
            args.extend(["-i".to_string(), format!("video={}", device_id)]);
        }
    }

    #[cfg(target_os = "linux")]
    {
        args.extend([
            "-f".to_string(),
            "v4l2".to_string(),
            "-i".to_string(),
            device_id.to_string(),
        ]);
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    { let _ = (device_id, opts); }

    args
}

fn rtmp_args(bind_address: &str, port: u16, application: &str, opts: &SourceArgsOptions) -> Vec<String> {
    let mut args = Vec::new();
    let host = if opts.include_rtmp_live && bind_address == "0.0.0.0" {
        "127.0.0.1"
    } else {
        bind_address
    };
    if opts.include_rtmp_live {
        args.extend(["-rtmp_live".to_string(), "live".to_string()]);
    }
    args.extend([
        "-i".to_string(),
        format!("rtmp://{}:{}/{}", host, port, application),
    ]);
    args
}

fn audio_device_args(device_id: &str) -> Vec<String> {
    #[cfg(target_os = "macos")]
    {
        vec![
            "-f".to_string(), "avfoundation".to_string(),
            "-i".to_string(), format!(":{}", device_id),
        ]
    }
    #[cfg(target_os = "windows")]
    {
        vec![
            "-f".to_string(), "dshow".to_string(),
            "-i".to_string(), format!("audio={}", device_id),
        ]
    }
    #[cfg(target_os = "linux")]
    {
        vec![
            "-f".to_string(), "pulse".to_string(),
            "-i".to_string(), device_id.to_string(),
        ]
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = device_id;
        vec![]
    }
}

fn window_capture_args(
    win: &crate::models::WindowCaptureSource,
    _opts: &SourceArgsOptions,
) -> Vec<String> {
    let mut args = Vec::new();

    #[cfg(target_os = "macos")]
    {
        args.extend([
            "-f".to_string(), "avfoundation".to_string(),
            "-framerate".to_string(), win.fps.to_string(),
            "-capture_cursor".to_string(),
            if win.capture_cursor { "1" } else { "0" }.to_string(),
            "-i".to_string(), format!("{}:none", win.window_id),
        ]);
    }

    #[cfg(target_os = "windows")]
    {
        args.extend([
            "-f".to_string(), "gdigrab".to_string(),
            "-framerate".to_string(), win.fps.to_string(),
            "-draw_mouse".to_string(),
            if win.capture_cursor { "1" } else { "0" }.to_string(),
            "-i".to_string(), format!("title={}", win.window_title),
        ]);
    }

    #[cfg(target_os = "linux")]
    {
        args.extend([
            "-f".to_string(), "x11grab".to_string(),
            "-framerate".to_string(), win.fps.to_string(),
            "-draw_mouse".to_string(),
            if win.capture_cursor { "1" } else { "0" }.to_string(),
            "-i".to_string(), format!(":0.0+{}", win.window_id),
        ]);
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    { let _ = win; }

    args
}

fn placeholder_args(color: &str, opts: &SourceArgsOptions) -> Vec<String> {
    vec![
        "-f".to_string(),
        "lavfi".to_string(),
        "-i".to_string(),
        format!(
            "color=c={}:s={}x{}:r=30:d=3600",
            color, opts.placeholder_width, opts.placeholder_height
        ),
    ]
}
