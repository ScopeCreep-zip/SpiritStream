// Compositor Service
// Generates FFmpeg filter_complex strings for scene composition

use crate::models::{Scene, Source, SourceLayer};

/// Compositor service for generating FFmpeg filter graphs
pub struct Compositor;

impl Compositor {
    /// Build FFmpeg input arguments for all sources in a scene
    /// Returns a vector of FFmpeg input arguments (e.g., ["-i", "rtmp://...", "-i", "file.mp4"])
    pub fn build_input_args(scene: &Scene, sources: &[Source]) -> Vec<String> {
        let mut args = Vec::new();

        // Get unique source IDs used in the scene's layers
        let used_source_ids: Vec<&str> = scene.layers
            .iter()
            .map(|l| l.source_id.as_str())
            .collect();

        // Build input arguments for each used source
        for source_id in &used_source_ids {
            if let Some(source) = sources.iter().find(|s| s.id() == *source_id) {
                args.extend(Self::source_to_input_args(source));
            }
        }

        args
    }

    /// Convert a source to FFmpeg input arguments (delegates to shared ffmpeg_source_args module)
    fn source_to_input_args(source: &Source) -> Vec<String> {
        // Compositor mode: no validation errors expected, unwrap is safe
        super::ffmpeg_source_args::source_input_args(source, &super::ffmpeg_source_args::COMPOSITOR_OPTS)
            .unwrap_or_default()
    }

    /// Build video filter_complex string for compositing multiple sources
    /// The scene's layers are composited in z-order (lowest z-index at bottom)
    pub fn build_video_filter(scene: &Scene, sources: &[Source]) -> String {
        let visible_layers: Vec<_> = scene.layers
            .iter()
            .filter(|l| l.visible)
            .collect();

        if visible_layers.is_empty() {
            // No visible layers - return empty canvas
            return format!(
                "color=c=black:s={}x{}:r=30[vout]",
                scene.canvas_width, scene.canvas_height
            );
        }

        if visible_layers.len() == 1 {
            // Single source - just scale and position
            let layer = visible_layers[0];
            let input_idx = Self::get_input_index(scene, sources, &layer.source_id);
            return Self::build_single_layer_filter(
                input_idx,
                layer,
                scene.canvas_width,
                scene.canvas_height,
            );
        }

        // Multiple sources - build composite filter
        Self::build_composite_filter(scene, sources, &visible_layers)
    }

    /// Build filter for a single layer
    fn build_single_layer_filter(
        input_idx: usize,
        layer: &SourceLayer,
        canvas_width: u32,
        canvas_height: u32,
    ) -> String {
        let t = &layer.transform;
        let mut filters = Vec::new();

        // Apply crop if specified
        if let Some(ref crop) = t.crop {
            filters.push(format!(
                "[{}:v]crop=w=in_w-{}-{}:h=in_h-{}-{}:x={}:y={}",
                input_idx,
                crop.left, crop.right,
                crop.top, crop.bottom,
                crop.left, crop.top
            ));
        } else {
            filters.push(format!("[{}:v]", input_idx));
        }

        // Scale to layer size
        filters.push(format!("scale={}:{}", t.width, t.height));

        // Apply rotation if specified
        if t.rotation != 0.0 {
            // Convert degrees to radians
            let radians = t.rotation * std::f32::consts::PI / 180.0;
            filters.push(format!("rotate={}:c=none", radians));
        }

        // Create canvas and overlay
        format!(
            "color=c=black:s={}x{}:r=30[canvas];{}[scaled];[canvas][scaled]overlay={}:{}[vout]",
            canvas_width, canvas_height,
            filters.join(","),
            t.x, t.y
        )
    }

    /// Build composite filter for multiple layers
    fn build_composite_filter(
        scene: &Scene,
        sources: &[Source],
        layers: &[&SourceLayer],
    ) -> String {
        let mut filter_parts = Vec::new();
        let mut sorted_layers = layers.to_vec();
        sorted_layers.sort_by_key(|l| l.z_index);

        // First, scale each input to its layer size
        for (i, layer) in sorted_layers.iter().enumerate() {
            let input_idx = Self::get_input_index(scene, sources, &layer.source_id);
            let t = &layer.transform;

            let mut source_filter = format!("[{}:v]", input_idx);

            // Apply crop if specified
            if let Some(ref crop) = t.crop {
                source_filter.push_str(&format!(
                    "crop=w=in_w-{}-{}:h=in_h-{}-{}:x={}:y={},",
                    crop.left, crop.right,
                    crop.top, crop.bottom,
                    crop.left, crop.top
                ));
            }

            // Scale to layer size
            source_filter.push_str(&format!("scale={}:{}[v{}]", t.width, t.height, i));

            filter_parts.push(source_filter);
        }

        // Create black canvas
        filter_parts.push(format!(
            "color=c=black:s={}x{}:r=30[canvas]",
            scene.canvas_width, scene.canvas_height
        ));

        // Chain overlay operations
        let mut current_label = "canvas".to_string();
        for (i, layer) in sorted_layers.iter().enumerate() {
            let t = &layer.transform;
            let next_label = if i == sorted_layers.len() - 1 {
                "vout".to_string()
            } else {
                format!("comp{}", i)
            };

            filter_parts.push(format!(
                "[{}][v{}]overlay={}:{}[{}]",
                current_label, i, t.x, t.y, next_label
            ));

            current_label = next_label;
        }

        filter_parts.join(";")
    }

    /// Build audio filter_complex string for mixing multiple sources
    /// Includes astats filter for real-time audio level monitoring
    pub fn build_audio_filter(scene: &Scene, sources: &[Source]) -> String {
        Self::build_audio_filter_with_metering(scene, sources, true)
    }

    /// Build audio filter with optional metering
    pub fn build_audio_filter_with_metering(scene: &Scene, sources: &[Source], enable_metering: bool) -> String {
        let mixer = &scene.audio_mixer;
        let active_tracks: Vec<_> = mixer.tracks
            .iter()
            .filter(|t| !t.muted)
            .collect();

        // Check for solo - if any track is soloed, only include soloed tracks
        let soloed_tracks: Vec<_> = active_tracks
            .iter()
            .filter(|t| t.solo)
            .cloned()
            .collect();

        let tracks_to_mix = if !soloed_tracks.is_empty() {
            soloed_tracks
        } else {
            active_tracks
        };

        // astats filter for audio level metering
        // metadata=1 outputs per-frame metadata, reset=1 resets stats each second
        // length=0.4 uses 400ms window for RMS calculation (smooth but responsive)
        let astats_filter = if enable_metering {
            ",astats=metadata=1:reset=1:length=0.4"
        } else {
            ""
        };

        if tracks_to_mix.is_empty() {
            // No audio - generate silence
            return format!("anullsrc=r=48000:cl=stereo{}[aout]", astats_filter);
        }

        // Effective master volume (0 if muted)
        let effective_master = if mixer.master_muted { 0.0 } else { mixer.master_volume };

        if tracks_to_mix.len() == 1 {
            // Single audio source
            let track = tracks_to_mix[0];
            let input_idx = Self::get_input_index(scene, sources, &track.source_id);
            let volume_filter = if track.volume != 1.0 || effective_master != 1.0 {
                format!("volume={}", track.volume * effective_master)
            } else {
                "anull".to_string() // passthrough
            };
            return format!("[{}:a]{}{}[aout]", input_idx, volume_filter, astats_filter);
        }

        // Multiple audio sources - mix them
        let mut filter_parts = Vec::new();
        let mut audio_labels = Vec::new();

        for (i, track) in tracks_to_mix.iter().enumerate() {
            let input_idx = Self::get_input_index(scene, sources, &track.source_id);
            let label = format!("a{}", i);

            // Apply per-track volume with effective master (0 if muted)
            let volume = track.volume * effective_master;
            filter_parts.push(format!(
                "[{}:a]volume={}[{}]",
                input_idx, volume, label
            ));
            audio_labels.push(format!("[{}]", label));
        }

        // Mix all audio sources and add astats metering
        filter_parts.push(format!(
            "{}amix=inputs={}:duration=longest{}[aout]",
            audio_labels.join(""),
            tracks_to_mix.len(),
            astats_filter
        ));

        filter_parts.join(";")
    }

    /// Build combined filter_complex for both video and audio
    pub fn build_filter_complex(scene: &Scene, sources: &[Source]) -> String {
        let video_filter = Self::build_video_filter(scene, sources);
        let audio_filter = Self::build_audio_filter(scene, sources);

        format!("{};{}", video_filter, audio_filter)
    }

    /// Get the FFmpeg input index for a source
    /// Returns the index based on the order sources appear in the scene's layers
    fn get_input_index(scene: &Scene, _sources: &[Source], source_id: &str) -> usize {
        let used_source_ids: Vec<&str> = scene.layers
            .iter()
            .map(|l| l.source_id.as_str())
            .collect();

        // Find the index of this source in the deduplicated list
        let mut seen = std::collections::HashSet::new();
        let mut index = 0;
        for id in used_source_ids {
            if !seen.contains(id) {
                if id == source_id {
                    return index;
                }
                seen.insert(id);
                index += 1;
            }
        }

        0 // Fallback to first input
    }

    /// Build complete FFmpeg arguments for scene-based streaming
    /// Returns all arguments needed for FFmpeg command (inputs + filter_complex + mapping)
    pub fn build_ffmpeg_args(
        scene: &Scene,
        sources: &[Source],
        output_url: &str,
    ) -> Vec<String> {
        let mut args = Vec::new();

        // Add input arguments
        args.extend(Self::build_input_args(scene, sources));

        // Add filter_complex
        let filter = Self::build_filter_complex(scene, sources);
        args.extend(["-filter_complex".to_string(), filter]);

        // Map outputs
        args.extend([
            "-map".to_string(), "[vout]".to_string(),
            "-map".to_string(), "[aout]".to_string(),
        ]);

        // Output format and URL
        args.extend([
            "-c:v".to_string(), "libx264".to_string(),
            "-preset".to_string(), "veryfast".to_string(),
            "-c:a".to_string(), "aac".to_string(),
            "-f".to_string(), "flv".to_string(),
            output_url.to_string(),
        ]);

        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{RtmpSource, MediaFileSource, AudioMixer, Transform};

    fn create_test_scene() -> Scene {
        Scene {
            id: "scene1".to_string(),
            name: "Test Scene".to_string(),
            canvas_width: 1920,
            canvas_height: 1080,
            layers: vec![
                SourceLayer {
                    id: "layer1".to_string(),
                    source_id: "rtmp1".to_string(),
                    visible: true,
                    locked: false,
                    transform: Transform {
                        x: 0,
                        y: 0,
                        width: 1920,
                        height: 1080,
                        rotation: 0.0,
                        crop: None,
                    },
                    z_index: 0,
                },
            ],
            audio_mixer: AudioMixer::default(),
            transition_in: None,
        }
    }

    fn create_test_sources() -> Vec<Source> {
        vec![
            Source::Rtmp(RtmpSource {
                id: "rtmp1".to_string(),
                name: "Main Input".to_string(),
                bind_address: "0.0.0.0".to_string(),
                port: 1935,
                application: "live".to_string(),
                capture_audio: true,
            }),
        ]
    }

    #[test]
    fn test_build_input_args() {
        let scene = create_test_scene();
        let sources = create_test_sources();

        let args = Compositor::build_input_args(&scene, &sources);

        assert!(args.contains(&"-i".to_string()));
        assert!(args.iter().any(|a| a.contains("rtmp://")));
    }

    #[test]
    fn test_build_video_filter_single_source() {
        let scene = create_test_scene();
        let sources = create_test_sources();

        let filter = Compositor::build_video_filter(&scene, &sources);

        assert!(filter.contains("scale=1920:1080"));
        assert!(filter.contains("[vout]"));
    }

    #[test]
    fn test_build_audio_filter() {
        let scene = create_test_scene();
        let sources = create_test_sources();

        let filter = Compositor::build_audio_filter(&scene, &sources);

        assert!(filter.contains("[aout]"));
    }

    #[test]
    fn test_build_video_filter_pip() {
        let mut scene = create_test_scene();

        // Add a second layer as PiP
        scene.layers.push(SourceLayer {
            id: "layer2".to_string(),
            source_id: "file1".to_string(),
            visible: true,
            locked: false,
            transform: Transform {
                x: 1400,
                y: 50,
                width: 480,
                height: 270,
                rotation: 0.0,
                crop: None,
            },
            z_index: 1,
        });

        let mut sources = create_test_sources();
        sources.push(Source::MediaFile(MediaFileSource {
            id: "file1".to_string(),
            name: "Overlay".to_string(),
            file_path: "/path/to/file.mp4".to_string(),
            loop_playback: true,
            audio_only: false,
            capture_audio: true,
        }));

        let filter = Compositor::build_video_filter(&scene, &sources);

        // Should have overlay operations
        assert!(filter.contains("overlay"));
        assert!(filter.contains("[vout]"));
    }
}
