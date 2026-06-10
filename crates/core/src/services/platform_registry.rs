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
        if !(url.starts_with("rtmp://") || url.starts_with("rtmps://")) {
            // The placement logic below assumes an RTMP `{app}/{key}` shape.
            // A custom HTTP(S) ingest can still carry the key in its path or
            // query, so defer to the scheme-agnostic redactor rather than
            // echoing the URL verbatim into the log.
            if url.starts_with("http://") || url.starts_with("https://") {
                return redact_non_rtmp_url(url);
            }
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

                    if url.starts_with(before_key) {
                        // Template ends in `{stream_key}` (empty suffix): the
                        // key runs to the end of the URL. `url.find("")`
                        // returns `Some(0)`, so the generic branch below would
                        // echo the entire URL — key included — into the log.
                        // Mask everything after the known prefix instead.
                        if after_key.is_empty() {
                            return format!("{before_key}***");
                        }
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

/// Redact credential material from a non-RTMP (`http`/`https`) stream URL.
///
/// HTTP ingests (e.g. a custom HLS upload endpoint) don't follow the RTMP
/// `{app}/{key}` path shape — the key may live in a query-string value
/// (`?cid=KEY`) or as the trailing path segment (`/live/KEY`). We mask both
/// defensively: every query-string *value* and the last path segment.
/// Over-redaction is the correct bias on a safety-critical log path — a leaked
/// stream key is a direct threat-model hit, a redacted endpoint name is not.
fn redact_non_rtmp_url(url: &str) -> String {
    let (scheme, rest) = match url.split_once("://") {
        Some(parts) => parts,
        None => return url.to_string(),
    };

    let (path_part, query_part) = match rest.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (rest, None),
    };

    let redacted_path = match path_part.split_once('/') {
        Some((host, path)) => {
            let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            if segments.is_empty() {
                host.to_string()
            } else {
                let kept = &segments[..segments.len() - 1];
                if kept.is_empty() {
                    format!("{host}/***")
                } else {
                    format!("{host}/{}/***", kept.join("/"))
                }
            }
        }
        None => path_part.to_string(),
    };

    match query_part {
        Some(query) => {
            let redacted_query = query
                .split('&')
                .map(|pair| match pair.split_once('=') {
                    Some((key, _)) => format!("{key}=***"),
                    None => pair.to_string(),
                })
                .collect::<Vec<_>>()
                .join("&");
            format!("{scheme}://{redacted_path}?{redacted_query}")
        }
        None => format!("{scheme}://{redacted_path}"),
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
        let data: PlatformsJson =
            serde_json::from_str(json_content).map_err(|e| CoreError::Internal {
                context: format!(
                    "platform registry: embedded streaming-platforms.json is malformed: {e}"
                ),
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
            if url.starts_with("http://") || url.starts_with("https://") {
                return redact_non_rtmp_url(url);
            }
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

    fn append_config(position: usize) -> PlatformConfig {
        PlatformConfig {
            name: "Test",
            default_server: "rtmp://host/app",
            placement: StreamKeyPlacement::Append,
            default_app_path: Some("app"),
            stream_key_position: position,
        }
    }

    #[test]
    fn display_name_returns_human_label() {
        assert_eq!(append_config(2).display_name(), "Test");
    }

    /// The safety-critical path: the trailing stream-key segment must be
    /// replaced with `***` so operator logs never carry the live key.
    #[test]
    fn redact_url_append_masks_trailing_key_segment() {
        let cfg = append_config(2);
        let redacted = cfg.redact_url("rtmp://host/app/SUPERSECRETKEY");
        assert_eq!(redacted, "rtmp://host/app/***");
        assert!(!redacted.contains("SUPERSECRETKEY"));
    }

    #[test]
    fn redact_url_append_position_zero_is_noop() {
        let cfg = append_config(0);
        let url = "rtmp://host/app/key";
        assert_eq!(cfg.redact_url(url), url);
    }

    /// An HTTP(S) target reaching the RTMP-shaped instance redactor must still
    /// be masked (custom HTTPS ingest), not echoed verbatim into a log.
    #[test]
    fn redact_url_masks_non_rtmp_urls() {
        let cfg = append_config(2);
        let redacted = cfg.redact_url("https://example.com/app/key");
        assert_eq!(redacted, "https://example.com/app/***");
        assert!(!redacted.ends_with("/key"));
    }

    #[test]
    fn redact_url_append_too_few_segments_is_noop() {
        let cfg = append_config(2);
        // Only one segment before the key position → nothing to redact.
        let url = "rtmp://host/onlyone";
        assert_eq!(cfg.redact_url(url), url);
    }

    /// Template placement with a suffix after `{stream_key}` redacts the
    /// key while preserving the suffix.
    #[test]
    fn redact_url_template_masks_key_keeping_suffix() {
        let cfg = PlatformConfig {
            name: "Tmpl",
            default_server: "rtmp://host/live2/{stream_key}/extra",
            placement: StreamKeyPlacement::InUrlTemplate,
            default_app_path: None,
            stream_key_position: 0,
        };
        let redacted = cfg.redact_url("rtmp://host/live2/SECRET/extra");
        assert_eq!(redacted, "rtmp://host/live2/***/extra");
        assert!(!redacted.contains("SECRET"));
    }

    /// A template that ends in `{stream_key}` (empty suffix) must still mask
    /// the key. `url.find("")` returns `Some(0)`, so a naive implementation
    /// echoes the whole URL — key included — into the log. Regression for F6.
    #[test]
    fn redact_url_template_empty_suffix_masks_key() {
        let cfg = PlatformConfig {
            name: "Tmpl",
            default_server: "rtmp://host/live2/{stream_key}",
            placement: StreamKeyPlacement::InUrlTemplate,
            default_app_path: None,
            stream_key_position: 0,
        };
        let redacted = cfg.redact_url("rtmp://host/live2/SUPERSECRETKEY");
        assert_eq!(redacted, "rtmp://host/live2/***");
        assert!(!redacted.contains("SUPERSECRETKEY"));
    }

    #[test]
    fn build_url_with_key_appends_for_append_platform() {
        let registry = PlatformRegistry::new().expect("test fixture");
        let url =
            registry.build_url_with_key(&crate::models::Platform::Twitch, "rtmp://host/app", "KEY");
        assert_eq!(url, "rtmp://host/app/KEY");
    }

    #[test]
    fn build_url_with_key_unknown_platform_falls_back_to_append() {
        let registry = PlatformRegistry::new().expect("test fixture");
        let url = registry.build_url_with_key(
            &crate::models::Platform::Custom,
            "rtmp://host/live/",
            "KEY",
        );
        assert_eq!(url, "rtmp://host/live/KEY");
    }

    /// HTTP(S) ingests can carry a stream key in the path or query string, so
    /// `generic_redact` must mask them rather than echo them into a log line.
    /// This matrix pins every shape an HTTPS custom target can take.
    #[test]
    fn generic_redact_masks_http_path_carried_key() {
        let redacted =
            PlatformRegistry::generic_redact("https://ingest.example.com/live/SUPERSECRET");
        assert_eq!(redacted, "https://ingest.example.com/live/***");
        assert!(!redacted.contains("SUPERSECRET"));
    }

    #[test]
    fn generic_redact_masks_http_query_carried_key() {
        // The shape the now-removed `YouTube - HLS` entry would have produced:
        // the key rides in the `cid` query value with an inline endpoint name.
        let url =
            "https://a.upload.youtube.com/http_upload_hls?cid=SUPERSECRET&copy=0&file=out.m3u8";
        let redacted = PlatformRegistry::generic_redact(url);
        assert!(!redacted.contains("SUPERSECRET"), "key leaked: {redacted}");
        assert_eq!(
            redacted,
            "https://a.upload.youtube.com/***?cid=***&copy=***&file=***"
        );
    }

    #[test]
    fn generic_redact_masks_http_single_segment_key() {
        let redacted = PlatformRegistry::generic_redact("http://host/SUPERSECRET");
        assert_eq!(redacted, "http://host/***");
        assert!(!redacted.contains("SUPERSECRET"));
    }

    /// A host-only HTTP(S) URL has nothing to redact and passes through.
    #[test]
    fn generic_redact_http_host_only_is_noop() {
        assert_eq!(
            PlatformRegistry::generic_redact("https://host"),
            "https://host"
        );
    }

    /// Non-stream schemes (no rtmp/http) are still left untouched.
    #[test]
    fn generic_redact_unknown_scheme_is_noop() {
        let url = "file:///etc/passwd";
        assert_eq!(PlatformRegistry::generic_redact(url), url);
    }

    /// The instance redactor must defer to the HTTP(S) path even though its
    /// placement template logic is RTMP-shaped — a config matched against an
    /// HTTPS target still can't leak the key into a log.
    #[test]
    fn redact_url_https_target_defers_to_http_redactor() {
        let cfg = append_config(2);
        let redacted = cfg.redact_url("https://host/live/SUPERSECRET?token=ALSOSECRET");
        assert!(
            !redacted.contains("SUPERSECRET"),
            "path key leaked: {redacted}"
        );
        assert!(
            !redacted.contains("ALSOSECRET"),
            "query key leaked: {redacted}"
        );
        assert_eq!(redacted, "https://host/live/***?token=***");
    }

    #[test]
    fn generic_redact_single_segment_is_noop() {
        let url = "rtmp://host/onlyone";
        assert_eq!(PlatformRegistry::generic_redact(url), url);
    }

    /// A bare value with no `scheme://` still gets the app path appended —
    /// the user may type just a hostname for a known platform.
    #[test]
    fn normalize_url_without_scheme_appends_app_path() {
        let cfg = append_config(2);
        assert_eq!(
            cfg.normalize_url("ingest.example.com"),
            "ingest.example.com/app"
        );
    }

    /// A host with a trailing slash (empty path) gets the app path filled in.
    #[test]
    fn normalize_url_trailing_slash_fills_app_path() {
        let cfg = append_config(2);
        assert_eq!(cfg.normalize_url("rtmp://host/"), "rtmp://host/app");
    }

    /// A scheme+host with no path slash at all gets the app path appended.
    #[test]
    fn normalize_url_host_only_appends_app_path() {
        let cfg = append_config(2);
        assert_eq!(cfg.normalize_url("rtmp://host"), "rtmp://host/app");
    }

    /// Append redaction on a host-only URL (no path segment to redact) must
    /// pass through untouched rather than panic on the missing segment.
    #[test]
    fn redact_url_append_host_only_is_noop() {
        let cfg = append_config(2);
        let url = "rtmp://hostonly";
        assert_eq!(cfg.redact_url(url), url);
    }

    /// Template redaction where the configured template does NOT match the
    /// incoming URL falls back to generic segment redaction — the last path
    /// segment is masked so the key never reaches a log line even when the
    /// template shape is unexpected.
    #[test]
    fn redact_url_template_non_matching_falls_back_to_generic() {
        let cfg = PlatformConfig {
            name: "Tmpl",
            default_server: "rtmp://other-host/{stream_key}",
            placement: StreamKeyPlacement::InUrlTemplate,
            default_app_path: None,
            stream_key_position: 0,
        };
        // URL host differs from the template host → primary match fails →
        // generic_segment_redact masks the trailing segment.
        let redacted = cfg.redact_url("rtmp://host/app/SUPERSECRET");
        assert_eq!(redacted, "rtmp://host/app/***");
        assert!(!redacted.contains("SUPERSECRET"));
    }

    #[test]
    fn extract_app_path_without_scheme_defaults() {
        assert_eq!(PlatformRegistry::extract_app_path("noscheme"), (None, 2));
    }

    #[test]
    fn extract_app_path_empty_path_defaults() {
        assert_eq!(
            PlatformRegistry::extract_app_path("rtmp://host/"),
            (None, 2)
        );
    }

    #[test]
    fn extract_app_path_takes_first_segment() {
        assert_eq!(
            PlatformRegistry::extract_app_path("rtmp://host/app/more/key"),
            (Some("app".to_string()), 2)
        );
    }

    #[test]
    fn generic_redact_host_only_is_noop() {
        let url = "rtmp://hostonly";
        assert_eq!(PlatformRegistry::generic_redact(url), url);
    }
}
