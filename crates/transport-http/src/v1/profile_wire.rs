//! Wire-mirror types for the profile surface — utoipa is transport-only,
//! so `ToSchema` lives on these mirrors rather than the core types.
//!
//! G5: replaces the prior `Json<serde_json::Value>` returns on
//! `v1_profile_show` + `v1_profile_activate` with a properly-typed
//! Profile tree. The JSON wire shape is byte-identical to the ts-rs
//! export at `@spiritstream/types/Profile`; consumers using the
//! generated TS SDK now get a concrete type instead of `unknown`.
//!
//! Same pattern as `v1/chat/wire.rs` (`ChatPlatformWire` etc) — every
//! struct/enum mirrors a core type, with `From<Core>` for the response
//! path. The `Platform` enum on `StreamTarget` is flattened to `String`
//! on the wire because it has ~80 generated variants (one per entry in
//! `data/streaming-platforms.json`); the TS side keeps the typed
//! `Platform` union from the ts-rs export, the OpenAPI schema is just
//! `type: string`. This is a deliberate tradeoff — the alternative is
//! mirroring 80 enum variants by hand.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use spiritstream_core::models::{
    AudioSettings, BackendSettings, ChatSettings, ContainerSettings, DiscordSettings, OAuthAccount,
    OAuthSettings, ObsIntegrationDirection, ObsSettings, OutputGroup, Profile, ProfileSettings,
    RtmpInput, StreamTarget, VideoSettings,
};

// ---------------------------------------------------------------------------
// RtmpInput
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RtmpInputWire {
    #[serde(rename = "type")]
    pub input_type: String,
    pub bind_address: String,
    pub port: u16,
    pub application: String,
    /// Server-computed ingest URL (`refresh_url`). Dropping this field
    /// from the mirror once shipped clients an input with NO url — the
    /// UI then started streams with an empty ingest URL and FFmpeg's
    /// relay died instantly. The drift guard below pins the full set.
    pub url: String,
}

impl From<RtmpInput> for RtmpInputWire {
    fn from(v: RtmpInput) -> Self {
        Self {
            input_type: v.input_type,
            bind_address: v.bind_address,
            port: v.port,
            application: v.application,
            url: v.url,
        }
    }
}

// ---------------------------------------------------------------------------
// Encoder settings
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VideoSettingsWire {
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate: String,
    pub preset: Option<String>,
    pub profile: Option<String>,
    pub keyframe_interval_seconds: Option<u32>,
}

impl From<VideoSettings> for VideoSettingsWire {
    fn from(v: VideoSettings) -> Self {
        Self {
            codec: v.codec,
            width: v.width,
            height: v.height,
            fps: v.fps,
            bitrate: v.bitrate,
            preset: v.preset,
            profile: v.profile,
            keyframe_interval_seconds: v.keyframe_interval_seconds,
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AudioSettingsWire {
    pub codec: String,
    pub bitrate: String,
    pub channels: u8,
    pub sample_rate: u32,
}

impl From<AudioSettings> for AudioSettingsWire {
    fn from(v: AudioSettings) -> Self {
        Self {
            codec: v.codec,
            bitrate: v.bitrate,
            channels: v.channels,
            sample_rate: v.sample_rate,
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContainerSettingsWire {
    pub format: String,
}

impl From<ContainerSettings> for ContainerSettingsWire {
    fn from(v: ContainerSettings) -> Self {
        Self { format: v.format }
    }
}

// ---------------------------------------------------------------------------
// StreamTarget (Platform → String on the wire; see module docs)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StreamTargetWire {
    pub id: String,
    /// Streaming platform name — matches a `displayName` in
    /// `data/streaming-platforms.json` or `"Custom"`. Flattened to
    /// `String` on the wire because the generated `Platform` enum has
    /// ~80 variants; the TS side reads the typed union from the ts-rs
    /// export at `@spiritstream/types/Platform`.
    pub service: String,
    pub name: String,
    pub url: String,
    pub stream_key: String,
    pub enabled: bool,
}

impl From<StreamTarget> for StreamTargetWire {
    fn from(v: StreamTarget) -> Self {
        // `Platform` derives `Serialize`; serialise to its JSON string
        // repr (a single string per the generated enum's serde shape)
        // and unwrap. The only failure mode would be a Platform variant
        // that serialises to a non-string — none exist by construction.
        let service = serde_json::to_value(&v.service)
            .ok()
            .and_then(|val| val.as_str().map(str::to_owned))
            .unwrap_or_else(|| "Custom".to_string());
        Self {
            id: v.id,
            service,
            name: v.name,
            url: v.url,
            stream_key: v.stream_key,
            enabled: v.enabled,
        }
    }
}

// ---------------------------------------------------------------------------
// OutputGroup
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OutputGroupWire {
    pub id: String,
    pub name: String,
    pub is_default: bool,
    pub generate_pts: bool,
    pub video: VideoSettingsWire,
    pub audio: AudioSettingsWire,
    pub container: ContainerSettingsWire,
    pub stream_targets: Vec<StreamTargetWire>,
    pub enabled: bool,
}

impl From<OutputGroup> for OutputGroupWire {
    fn from(v: OutputGroup) -> Self {
        Self {
            id: v.id,
            name: v.name,
            is_default: v.is_default,
            generate_pts: v.generate_pts,
            video: v.video.into(),
            audio: v.audio.into(),
            container: v.container.into(),
            enabled: v.enabled,
            stream_targets: v.stream_targets.into_iter().map(Into::into).collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// ProfileSettings sub-trees
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BackendSettingsWire {
    pub remote_enabled: bool,
    pub ui_enabled: bool,
    pub host: String,
    pub port: u16,
    pub token: String,
}

impl From<BackendSettings> for BackendSettingsWire {
    fn from(v: BackendSettings) -> Self {
        Self {
            remote_enabled: v.remote_enabled,
            ui_enabled: v.ui_enabled,
            host: v.host,
            port: v.port,
            token: v.token,
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ObsIntegrationDirectionWire {
    ObsToSpiritstream,
    SpiritstreamToObs,
    Bidirectional,
    Disabled,
}

impl From<ObsIntegrationDirection> for ObsIntegrationDirectionWire {
    fn from(v: ObsIntegrationDirection) -> Self {
        match v {
            ObsIntegrationDirection::ObsToSpiritstream => Self::ObsToSpiritstream,
            ObsIntegrationDirection::SpiritstreamToObs => Self::SpiritstreamToObs,
            ObsIntegrationDirection::Bidirectional => Self::Bidirectional,
            ObsIntegrationDirection::Disabled => Self::Disabled,
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ObsSettingsWire {
    pub host: String,
    pub port: u16,
    pub password: String,
    pub use_auth: bool,
    pub direction: ObsIntegrationDirectionWire,
    pub auto_connect: bool,
}

impl From<ObsSettings> for ObsSettingsWire {
    fn from(v: ObsSettings) -> Self {
        Self {
            host: v.host,
            port: v.port,
            password: v.password,
            use_auth: v.use_auth,
            direction: v.direction.into(),
            auto_connect: v.auto_connect,
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DiscordSettingsWire {
    pub webhook_enabled: bool,
    pub webhook_url: String,
    pub go_live_message: String,
    pub cooldown_enabled: bool,
    pub cooldown_seconds: u32,
    pub image_path: String,
}

impl From<DiscordSettings> for DiscordSettingsWire {
    fn from(v: DiscordSettings) -> Self {
        Self {
            webhook_enabled: v.webhook_enabled,
            webhook_url: v.webhook_url,
            go_live_message: v.go_live_message,
            cooldown_enabled: v.cooldown_enabled,
            cooldown_seconds: v.cooldown_seconds,
            image_path: v.image_path,
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatSettingsWire {
    pub twitch_channel: String,
    pub youtube_channel_id: String,
    pub trovo_channel_id: String,
    pub kick_channel: String,
    pub tiktok_username: String,
    pub facebook_live_video_id: String,
    pub youtube_api_key: String,
    pub twitch_send_enabled: bool,
    pub youtube_send_enabled: bool,
    pub trovo_send_enabled: bool,
    pub kick_send_enabled: bool,
    pub send_all_enabled: bool,
    pub crosspost_enabled: bool,
    pub youtube_use_api_key: bool,
    pub visible_platforms: Vec<String>,
    pub visibility_panel_collapsed: bool,
    /// Apply follower-only chat at Twitch connect. Dropping this from
    /// the mirror made a plain UI round-trip silently DISABLE it —
    /// unacceptable for the harassment-prone users the flag protects.
    pub follower_only_default: bool,
}

impl From<ChatSettings> for ChatSettingsWire {
    fn from(v: ChatSettings) -> Self {
        Self {
            twitch_channel: v.twitch_channel,
            youtube_channel_id: v.youtube_channel_id,
            trovo_channel_id: v.trovo_channel_id,
            kick_channel: v.kick_channel,
            tiktok_username: v.tiktok_username,
            facebook_live_video_id: v.facebook_live_video_id,
            youtube_api_key: v.youtube_api_key,
            twitch_send_enabled: v.twitch_send_enabled,
            youtube_send_enabled: v.youtube_send_enabled,
            trovo_send_enabled: v.trovo_send_enabled,
            kick_send_enabled: v.kick_send_enabled,
            send_all_enabled: v.send_all_enabled,
            crosspost_enabled: v.crosspost_enabled,
            youtube_use_api_key: v.youtube_use_api_key,
            visible_platforms: v.visible_platforms,
            visibility_panel_collapsed: v.visibility_panel_collapsed,
            follower_only_default: v.follower_only_default,
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OAuthAccountWire {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
    pub user_id: String,
    pub username: String,
    pub display_name: String,
}

impl From<OAuthAccount> for OAuthAccountWire {
    fn from(v: OAuthAccount) -> Self {
        Self {
            access_token: v.access_token,
            refresh_token: v.refresh_token,
            expires_at: v.expires_at,
            user_id: v.user_id,
            username: v.username,
            display_name: v.display_name,
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OAuthSettingsWire {
    pub twitch: OAuthAccountWire,
    pub youtube: OAuthAccountWire,
    pub kick: OAuthAccountWire,
    pub facebook: OAuthAccountWire,
    pub trovo: OAuthAccountWire,
}

impl From<OAuthSettings> for OAuthSettingsWire {
    fn from(v: OAuthSettings) -> Self {
        Self {
            twitch: v.twitch.into(),
            youtube: v.youtube.into(),
            kick: v.kick.into(),
            facebook: v.facebook.into(),
            trovo: v.trovo.into(),
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSettingsWire {
    pub theme_id: String,
    pub language: String,
    pub show_notifications: bool,
    pub encrypt_stream_keys: bool,
    pub backend: BackendSettingsWire,
    pub obs: ObsSettingsWire,
    pub discord: DiscordSettingsWire,
    pub chat: ChatSettingsWire,
    pub oauth: OAuthSettingsWire,
}

impl From<ProfileSettings> for ProfileSettingsWire {
    fn from(v: ProfileSettings) -> Self {
        Self {
            theme_id: v.theme_id,
            language: v.language,
            show_notifications: v.show_notifications,
            encrypt_stream_keys: v.encrypt_stream_keys,
            backend: v.backend.into(),
            obs: v.obs.into(),
            discord: v.discord.into(),
            chat: v.chat.into(),
            oauth: v.oauth.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Profile (top-level)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProfileWire {
    pub id: String,
    pub name: String,
    pub encrypted: bool,
    pub input: RtmpInputWire,
    pub output_groups: Vec<OutputGroupWire>,
    pub settings: ProfileSettingsWire,
    pub pii_blocklist: Vec<String>,
    pub pii_fuzzy: bool,
    pub anonymous_logging: bool,
    pub anonymous_salt: String,
}

impl From<Profile> for ProfileWire {
    fn from(v: Profile) -> Self {
        Self {
            id: v.id,
            name: v.name,
            encrypted: v.encrypted,
            input: v.input.into(),
            output_groups: v.output_groups.into_iter().map(Into::into).collect(),
            settings: v.settings.into(),
            pii_blocklist: v.pii_blocklist,
            pii_fuzzy: v.pii_fuzzy,
            anonymous_logging: v.anonymous_logging,
            anonymous_salt: v.anonymous_salt,
        }
    }
}

#[cfg(test)]
mod drift_guard {
    use super::*;

    /// Recursively collect every object key path in a JSON value.
    fn key_paths(value: &serde_json::Value, prefix: &str, out: &mut Vec<String>) {
        match value {
            serde_json::Value::Object(map) => {
                for (k, v) in map {
                    let path = if prefix.is_empty() {
                        k.clone()
                    } else {
                        format!("{prefix}.{k}")
                    };
                    out.push(path.clone());
                    key_paths(v, &path, out);
                }
            }
            serde_json::Value::Array(items) => {
                if let Some(first) = items.first() {
                    key_paths(first, &format!("{prefix}[]"), out);
                }
            }
            _ => {}
        }
    }

    /// The wire mirror MUST stay key-for-key identical to the core
    /// model's own serde shape (which is what ts-rs exports to the
    /// frontend's types). This mirror is hand-maintained and HAS
    /// drifted before: `input.url`, both `enabled` flags, and
    /// `chat.followerOnlyDefault` were silently absent from every HTTP
    /// response while the TS types claimed they existed — the UI then
    /// started streams with an empty ingest URL. Any new model field
    /// fails this test until the mirror carries it.
    #[test]
    fn profile_wire_matches_core_profile_serde_shape() {
        // Fully-populated profile via serde defaults: one group with
        // one target so the nested array element shapes are exercised.
        let profile: Profile = serde_json::from_value(serde_json::json!({
            "id": "drift",
            "name": "drift",
            "encrypted": false,
            "input": {
                "type": "rtmp",
                "bindAddress": "127.0.0.1",
                "port": 1935,
                "application": "live"
            },
            "outputGroups": [{
                "id": "g1",
                "name": "g1",
                "isDefault": true,
                "generatePts": false,
                "video": {
                    "codec": "copy", "width": 1920, "height": 1080,
                    "fps": 60, "bitrate": "6000k", "preset": "veryfast",
                    "profile": "high", "keyframeInterval": 2
                },
                "audio": { "codec": "copy", "bitrate": "160k", "channels": 2, "sampleRate": 48000 },
                "container": { "format": "flv" },
                "streamTargets": [{
                    "id": "t1",
                    "service": "Custom",
                    "name": "t1",
                    "url": "rtmp://example/live",
                    "streamKey": "k"
                }]
            }]
        }))
        .expect("core Profile deserializes with serde defaults");

        let model_json = serde_json::to_value(&profile).expect("model serializes");
        let wire_json = serde_json::to_value(ProfileWire::from(profile)).expect("wire serializes");

        let mut model_keys = Vec::new();
        let mut wire_keys = Vec::new();
        key_paths(&model_json, "", &mut model_keys);
        key_paths(&wire_json, "", &mut wire_keys);
        model_keys.sort();
        wire_keys.sort();

        let missing: Vec<_> = model_keys
            .iter()
            .filter(|k| !wire_keys.contains(k))
            .collect();
        let extra: Vec<_> = wire_keys
            .iter()
            .filter(|k| !model_keys.contains(k))
            .collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "ProfileWire drifted from the core Profile serde shape.\n  missing from wire: {missing:?}\n  extra on wire: {extra:?}"
        );
    }
}
