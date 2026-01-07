// Platform Registry
// Centralized configuration for streaming platforms

use crate::models::Platform;
use std::collections::HashMap;

/// Platform-specific configuration
#[derive(Debug, Clone)]
pub struct PlatformConfig {
    /// Display name
    pub name: &'static str,

    /// Default RTMP server URL (without stream key)
    pub default_server: &'static str,

    /// Default app path (e.g., "app", "live2", "rtmp")
    /// Used for URL normalization
    pub default_app_path: Option<&'static str>,

    /// Stream key position in URL path (0 = no masking, 1 = /KEY, 2 = /app/KEY, etc.)
    pub stream_key_position: usize,
}

impl PlatformConfig {
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
            format!("{scheme}://{host}/{app_path}/{}", path.trim_start_matches('/'))
        } else {
            url.to_string()
        }
    }

    /// Redact stream key from URL for logging
    pub fn redact_url(&self, url: &str) -> String {
        // If stream key position is 0, don't redact
        if self.stream_key_position == 0 {
            return url.to_string();
        }

        // Only redact RTMP(S) URLs
        if !(url.starts_with("rtmp://") || url.starts_with("rtmps://")) {
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

/// Global platform registry
pub struct PlatformRegistry {
    configs: HashMap<Platform, PlatformConfig>,
}

impl PlatformRegistry {
    /// Create a new registry with default platform configurations
    pub fn new() -> Self {
        let mut configs = HashMap::new();

        // YouTube
        configs.insert(Platform::Youtube, PlatformConfig {
            name: "YouTube",
            default_server: "rtmp://a.rtmp.youtube.com/live2",
            default_app_path: Some("live2"),
            stream_key_position: 2, // /live2/KEY
        });

        // Twitch
        configs.insert(Platform::Twitch, PlatformConfig {
            name: "Twitch",
            default_server: "rtmp://ingest.global-contribute.live-video.net/app",
            default_app_path: Some("app"),
            stream_key_position: 2, // /app/KEY
        });

        // Kick
        configs.insert(Platform::Kick, PlatformConfig {
            name: "Kick",
            default_server: "rtmps://fa723fc1b171.global-contribute.live-video.net/app",
            default_app_path: Some("app"),
            stream_key_position: 2, // /app/KEY
        });

        // Facebook
        configs.insert(Platform::Facebook, PlatformConfig {
            name: "Facebook Live",
            default_server: "rtmps://live-api-s.facebook.com:443/rtmp",
            default_app_path: Some("rtmp"),
            stream_key_position: 2, // /rtmp/KEY
        });

        // Custom (no normalization, use generic redaction)
        configs.insert(Platform::Custom, PlatformConfig {
            name: "Custom RTMP",
            default_server: "",
            default_app_path: None,
            stream_key_position: 2, // Generic: /app/KEY pattern
        });

        Self { configs }
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

    /// Redact stream key from URL
    pub fn redact_url(&self, platform: &Platform, url: &str) -> String {
        self.get(platform)
            .map(|config| config.redact_url(url))
            .unwrap_or_else(|| Self::generic_redact(url))
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

impl Default for PlatformRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_youtube_redaction() {
        let registry = PlatformRegistry::new();
        let url = "rtmp://a.rtmp.youtube.com/live2/my_secret_key_12345";
        let redacted = registry.redact_url(&Platform::Youtube, url);
        assert_eq!(redacted, "rtmp://a.rtmp.youtube.com/live2/***");
    }

    #[test]
    fn test_twitch_redaction() {
        let registry = PlatformRegistry::new();
        let url = "rtmp://ingest.global-contribute.live-video.net/app/live_123456_AbCdEfGhIjKlMnOp";
        let redacted = registry.redact_url(&Platform::Twitch, url);
        assert_eq!(redacted, "rtmp://ingest.global-contribute.live-video.net/app/***");
    }

    #[test]
    fn test_kick_redaction() {
        let registry = PlatformRegistry::new();
        let url = "rtmps://fa723fc1b171.global-contribute.live-video.net/app/kick_stream_key_xyz";
        let redacted = registry.redact_url(&Platform::Kick, url);
        assert_eq!(redacted, "rtmps://fa723fc1b171.global-contribute.live-video.net/app/***");
    }

    #[test]
    fn test_facebook_redaction() {
        let registry = PlatformRegistry::new();
        let url = "rtmps://live-api-s.facebook.com:443/rtmp/facebook_key_12345";
        let redacted = registry.redact_url(&Platform::Facebook, url);
        assert_eq!(redacted, "rtmps://live-api-s.facebook.com:443/rtmp/***");
    }

    #[test]
    fn test_kick_url_normalization() {
        let registry = PlatformRegistry::new();

        // URL without /app should get it added
        let url = "rtmps://fa723fc1b171.global-contribute.live-video.net";
        let normalized = registry.normalize_url(&Platform::Kick, url);
        assert_eq!(normalized, "rtmps://fa723fc1b171.global-contribute.live-video.net/app");

        // URL with /app should stay the same
        let url = "rtmps://fa723fc1b171.global-contribute.live-video.net/app";
        let normalized = registry.normalize_url(&Platform::Kick, url);
        assert_eq!(normalized, "rtmps://fa723fc1b171.global-contribute.live-video.net/app");
    }

    #[test]
    fn test_custom_no_redaction() {
        let registry = PlatformRegistry::new();
        let url = "rtmp://custom-server.com/stream/my_key";
        let redacted = registry.redact_url(&Platform::Custom, url);
        // Custom should not redact (stream_key_position = 0), so generic fallback applies
        assert_eq!(redacted, "rtmp://custom-server.com/stream/***");
    }
}
