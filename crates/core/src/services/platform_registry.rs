// Platform Registry
// Centralized configuration for streaming platforms

use crate::errors::CoreError;
use crate::models::Platform;
use serde::Deserialize;
use std::collections::HashMap;

/// Stream key placement strategy
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StreamKeyPlacement {
    /// Append stream key to URL (e.g., rtmp://server/app/{key})
    Append,
    /// Replace {stream_key} template in URL (e.g., rtmp://server/app/{stream_key})
    InUrlTemplate,
}

/// Platform-specific configuration
#[derive(Debug, Clone)]
pub struct PlatformConfig {
    /// Display name (human-readable, e.g. "YouTube", "Twitch")
    pub name: &'static str,

    /// Default RTMP server URL (may contain {stream_key} template)
    pub default_server: &'static str, // Used in InUrlTemplate redaction

    /// Stream key placement strategy
    pub placement: StreamKeyPlacement,

    /// Default app path (e.g., "app", "live2", "rtmp")
    /// Used for URL normalization
    pub default_app_path: Option<&'static str>,

    /// Stream key position in URL path (0 = no masking, 1 = /KEY, 2 = /app/KEY, etc.)
    pub stream_key_position: usize, // Used in Append redaction
}

impl PlatformConfig {
    /// Human-readable platform name (e.g., "YouTube", "Twitch"). Used
    /// by `redact_url_labeled` so operator logs show *which* target
    /// they're looking at when multiple stream destinations are
    /// running in parallel.
    pub fn display_name(&self) -> &'static str {
        self.name
    }

    /// Normalize a platform URL (e.g., ensure Kick has /app path)
    pub fn normalize_url(&self, url: &str) -> String {
        // If no default app path, no normalization needed
        let Some(app_path) = self.default_app_path else {
            return url.to_string();
        };

        // Parse the URL
        let (scheme, rest) = match url.split_once("://") {
            Some(parts) => parts,
            None => return format!("{url}/{app_path}"),
        };

        let (host, path) = match rest.split_once('/') {
            Some(parts) => parts,
            None => return format!("{scheme}://{rest}/{app_path}"),
        };

        // If path is empty or doesn't contain the app path, add it
        if path.is_empty() {
            format!("{scheme}://{host}/{app_path}")
        } else if !path.starts_with(app_path) {
            format!(
                "{scheme}://{host}/{app_path}/{}",
                path.trim_start_matches('/')
            )
        } else {
            url.to_string()
        }
    }

    /// Redact stream key from URL for logging
    pub fn redact_url(&self, url: &str) -> String {
        // Only redact RTMP(S) URLs
        if !(url.starts_with("rtmp://") || url.starts_with("rtmps://")) {
            return url.to_string();
        }

        match self.placement {
            StreamKeyPlacement::InUrlTemplate => {
                // For template mode, find where the template was and redact that portion
                // Template contains {stream_key}, so we need to find what replaced it
                let template = self.default_server;

                // Find the {stream_key} placeholder position
                if let Some(template_start) = template.find("{stream_key}") {
                    let before_key = &template[..template_start];
                    let after_key = &template[template_start + "{stream_key}".len()..];

                    // Check if URL matches the template pattern
                    if url.starts_with(before_key) && url.contains(after_key) {
                        // Find where the key ends (where after_key starts in the URL)
                        if let Some(key_end) = url.find(after_key) {
                            return format!("{}***{}", before_key, &url[key_end..]);
                        }
                    }
                }

                // Fallback: couldn't parse template, return generic redaction
                Self::generic_segment_redact(url)
            }
            StreamKeyPlacement::Append => {
                // For append mode, use path-based redaction
                if self.stream_key_position == 0 {
                    return url.to_string();
                }

                // Parse URL
                let (scheme, rest) = match url.split_once("://") {
                    Some(parts) => parts,
                    None => return url.to_string(),
                };

                let (host, path) = match rest.split_once('/') {
                    Some(parts) => parts,
                    None => return url.to_string(),
                };

                // Split path into segments
                let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

                // Check if we have enough segments to redact
                if segments.len() < self.stream_key_position {
                    return url.to_string();
                }

                // Build redacted URL
                let safe_segments = &segments[0..self.stream_key_position - 1];
                let safe_path = safe_segments.join("/");

                format!("{scheme}://{host}/{safe_path}/***")
            }
        }
    }

    /// Generic segment-based redaction (fallback)
    fn generic_segment_redact(url: &str) -> String {
        let (scheme, rest) = match url.split_once("://") {
            Some(parts) => parts,
            None => return url.to_string(),
        };

        let (host, path) = match rest.split_once('/') {
            Some(parts) => parts,
            None => return url.to_string(),
        };

        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if segments.len() < 2 {
            return url.to_string();
        }

        let safe_segments = &segments[0..segments.len() - 1];
        let safe_path = safe_segments.join("/");

        format!("{scheme}://{host}/{safe_path}/***")
    }
}

/// Global platform registry
pub struct PlatformRegistry {
    configs: HashMap<Platform, PlatformConfig>,
}

/// Wire-shape mirror of `data/streaming-platforms.json`. Typed-serde
/// parse so a malformed embedded data file fails as one structured
/// `CoreError::Internal` at startup rather than seven separate panics
/// scattered across `serde_json::Value`-tree navigation. The matching
/// `build.rs` deserializes the same file (different consumer — generates
/// the `Platform` enum) so any structural drift is caught at compile time
/// AND the runtime read-back still validates the contract.
#[derive(Deserialize)]
struct PlatformsJson {
    services: Vec<PlatformJsonEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlatformJsonEntry {
    name: String,
    #[serde(default)]
    display_name: Option<String>,
    default_url: String,
    stream_key_placement: String,
}

impl PlatformRegistry {
    /// Create a new registry by loading platform configurations from JSON.
    /// Returns `CoreError::Internal` on malformed embedded data (compile-time
    /// bug — `build.rs` validates the same schema, so a reaching this branch
    /// at runtime means the build pipeline regressed).
    pub fn new() -> Result<Self, CoreError> {
        let json_content = include_str!("../../../../data/streaming-platforms.json");
        let data: PlatformsJson = serde_json::from_str(json_content).map_err(|e| {
            CoreError::Internal {
                context: format!(
                    "platform registry: embedded streaming-platforms.json is malformed: {e}"
                ),
            }
        })?;

        let mut configs = HashMap::new();
        for service in data.services {
            // Filter: only include RTMP/RTMPS with "append" or "in_url_template" placement.
            if !service.default_url.starts_with("rtmp://")
                && !service.default_url.starts_with("rtmps://")
            {
                continue;
            }
            let placement = match service.stream_key_placement.as_str() {
                "append" => StreamKeyPlacement::Append,
                "in_url_template" => StreamKeyPlacement::InUrlTemplate,
                // Unknown placement variant — skip the entry so a new field
                // value added to the data file doesn't refuse startup.
                _ => continue,
            };

            // Skip entries whose `name` isn't a known `Platform` enum variant.
            // The `Platform` enum is generated by `build.rs` from the same JSON
            // — a mismatch here is impossible during a clean build, but
            // skipping (vs panicking) keeps `Platform::Custom` and any future
            // out-of-band variants from breaking startup.
            let Ok(platform) = serde_json::from_str::<Platform>(&format!("\"{}\"", service.name))
            else {
                continue;
            };

            let (app_path, stream_key_position) = Self::extract_app_path(&service.default_url);

            // Box::leak to create 'static strings. The registry is constructed
            // exactly once per process via ServiceRegistry::build, so this is
            // a one-time leak bounded by the data file size (~3KB).
            let display_name = service.display_name.as_deref().unwrap_or(&service.name);
            let static_display_name = Box::leak(display_name.to_string().into_boxed_str());
            let static_default_url = Box::leak(service.default_url.into_boxed_str());
            let static_app_path = app_path.map(|s| Box::leak(s.into_boxed_str()) as &'static str);

            configs.insert(
                platform,
                PlatformConfig {
                    name: static_display_name,
                    default_server: static_default_url,
                    placement,
                    default_app_path: static_app_path,
                    stream_key_position,
                },
            );
        }

        Ok(Self { configs })
    }

    /// Extract app path from RTMP URL
    /// Returns (Option<String>, stream_key_position)
    fn extract_app_path(url: &str) -> (Option<String>, usize) {
        // Parse URL to extract path
        let (_scheme, rest) = match url.split_once("://") {
            Some(parts) => parts,
            None => return (None, 2), // Default
        };

        let (_host, path) = match rest.split_once('/') {
            Some(parts) => parts,
            None => return (None, 2), // No path, default
        };

        // Extract first segment of path as app path
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        if segments.is_empty() {
            (None, 2)
        } else {
            (Some(segments[0].to_string()), 2) // Stream key is at position 2 (/app/KEY)
        }
    }

    /// Get configuration for a platform
    pub fn get(&self, platform: &Platform) -> Option<&PlatformConfig> {
        self.configs.get(platform)
    }

    /// Normalize URL for a platform
    pub fn normalize_url(&self, platform: &Platform, url: &str) -> String {
        self.get(platform)
            .map(|config| config.normalize_url(url))
            .unwrap_or_else(|| url.to_string())
    }

    /// Build complete URL with stream key based on platform's placement strategy
    pub fn build_url_with_key(
        &self,
        platform: &Platform,
        base_url: &str,
        stream_key: &str,
    ) -> String {
        if let Some(config) = self.get(platform) {
            match config.placement {
                StreamKeyPlacement::Append => {
                    // Append stream key to URL: rtmp://server/app + /key
                    format!("{}/{}", base_url.trim_end_matches('/'), stream_key)
                }
                StreamKeyPlacement::InUrlTemplate => {
                    // Replace {stream_key} template in URL
                    base_url.replace("{stream_key}", stream_key)
                }
            }
        } else {
            // Fallback: assume append mode
            format!("{}/{}", base_url.trim_end_matches('/'), stream_key)
        }
    }

    /// Generic redaction for unknown platforms (fallback)
    /// This is a public static method that can be used when platform context is not available
    pub fn generic_redact(url: &str) -> String {
        if !(url.starts_with("rtmp://") || url.starts_with("rtmps://")) {
            return url.to_string();
        }

        let (scheme, rest) = match url.split_once("://") {
            Some(parts) => parts,
            None => return url.to_string(),
        };

        let (host, path) = match rest.split_once('/') {
            Some(parts) => parts,
            None => return url.to_string(),
        };

        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if segments.len() < 2 {
            return url.to_string();
        }

        format!("{scheme}://{host}/{}/***", segments[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generic_redaction() {
        let url = "rtmp://custom-server.com/stream/my_key";
        let redacted = PlatformRegistry::generic_redact(url);
        assert_eq!(redacted, "rtmp://custom-server.com/stream/***");
    }

    #[test]
    fn test_registry_loads_from_json() {
        let registry = PlatformRegistry::new().expect("test fixture");
        // Verify registry is not empty
        assert!(
            !registry.configs.is_empty(),
            "Registry should load platforms from JSON"
        );
    }

    /// URL normalization must run on `ProfileService::save` so the
    /// persisted profile contains a usable RTMP URL regardless of what the
    /// user typed. This test pins the contract — a user typing just the
    /// hostname for a known platform must get the default app path appended.
    #[test]
    fn normalize_url_appends_default_app_path_for_twitch() {
        let registry = PlatformRegistry::new().expect("test fixture");
        let normalized = registry.normalize_url(
            &crate::models::Platform::Twitch,
            "rtmp://ingest.global-contribute.live-video.net",
        );
        assert_eq!(
            normalized, "rtmp://ingest.global-contribute.live-video.net/app",
            "twitch default_app_path must be appended when missing"
        );
    }

    /// Already-normalized URLs must pass through untouched (idempotent).
    #[test]
    fn normalize_url_is_idempotent_when_app_path_present() {
        let registry = PlatformRegistry::new().expect("test fixture");
        let already = "rtmp://ingest.global-contribute.live-video.net/app";
        assert_eq!(
            registry.normalize_url(&crate::models::Platform::Twitch, already),
            already
        );
    }

    /// Unknown / custom platforms have no normalization rules — URL is
    /// returned unchanged so the user can stream to arbitrary destinations.
    #[test]
    fn normalize_url_passes_through_for_custom_platform() {
        let registry = PlatformRegistry::new().expect("test fixture");
        let url = "rtmp://my-private-rtmp.example.com:1935/live";
        assert_eq!(
            registry.normalize_url(&crate::models::Platform::Custom, url),
            url
        );
    }
}
