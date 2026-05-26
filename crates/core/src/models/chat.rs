use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// Chat-log session status returned by `GET /api/v1/chat/log`.
/// `started_at` is Unix-epoch milliseconds; `0` when no session has
/// started since the last process boot (`active == false`). Sentinel
/// rather than `Option<i64>` to keep the ts-rs-emitted wire shape a
/// plain `bigint` instead of `bigint | null`, matching the rest of
/// `@spiritstream/types`'s timestamp fields.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct ChatLogStatus {
    pub active: bool,
    /// Unix epoch ms. Emitted as JSON number; ts-rs would default to
    /// `bigint` for i64 but `JSON.stringify(BigInt)` throws, so we
    /// pin the TS type to `number` (safe to year ~287,000 AD).
    #[serde(default)]
    #[ts(type = "number")]
    pub started_at: i64,
}

/// Represents a platform that supports chat
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub enum ChatPlatform {
    Twitch,
    #[serde(rename = "tiktok")]
    TikTok,
    YouTube,
    Trovo,
    Stripchat,
    Kick,
    Facebook,
}

impl ChatPlatform {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChatPlatform::Twitch => "twitch",
            ChatPlatform::TikTok => "tiktok",
            ChatPlatform::YouTube => "youtube",
            ChatPlatform::Trovo => "trovo",
            ChatPlatform::Stripchat => "stripchat",
            ChatPlatform::Kick => "kick",
            ChatPlatform::Facebook => "facebook",
        }
    }

    /// Maximum chat-message length the platform accepts. Enforced server-side
    /// in `ChatService::send_message`  before the PII filter
    /// runs, so over-length messages never touch the wire.
    pub fn max_message_chars(&self) -> usize {
        match self {
            ChatPlatform::Twitch => 500,
            ChatPlatform::YouTube => 200,
            ChatPlatform::Trovo => 500,
            ChatPlatform::Kick => 500,
            ChatPlatform::Facebook => 200,
            ChatPlatform::TikTok => 150,
            ChatPlatform::Stripchat => 500,
        }
    }
}

/// A normalized chat message from any platform.
///
/// **Field stratification.** `username` / `message` / `timestamp` /
/// `color` / `badges` are the legacy surface that pre-Phase-B chat-log
/// JSONL files were written with — they're preserved so the log-replay
/// path keeps round-tripping. New code reads `author` (structured
/// identity), `fragments` (renderable list), `flags` (boolean
/// attributes), and `event` (non-text events). Connectors that haven't
/// migrated yet leave the new fields at their defaults; the renderer
/// falls back to legacy fields when `fragments` is empty.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct ChatMessage {
    /// Unique message ID
    pub id: String,
    /// Platform this message came from
    pub platform: ChatPlatform,
    /// Phase J multi-account tuple key — the platform-side account
    /// identifier this message belongs to. `None` until that phase
    /// lands; connectors that already know the account ID may set it.
    #[serde(default)]
    pub account_id: Option<String>,
    /// Platform-specific room/chat ID (Twitch broadcaster_user_id,
    /// YouTube live_chat_id, …). `None` for the legacy log path.
    #[serde(default)]
    pub channel_id: Option<String>,
    /// Platforms this message was sent to (for cross-post / outbound)
    pub platforms: Option<Vec<ChatPlatform>>,
    /// Legacy: mirrors `author.display_name`. New code reads `author`.
    pub username: String,
    /// Legacy: raw message text. New code reads `fragments` / `raw_text`.
    pub message: String,
    /// Timestamp in milliseconds since Unix epoch (JSON `number`,
    /// safe through year ~287,000 AD).
    #[ts(type = "number")]
    pub timestamp: i64,
    /// Server-side receive time (separate from `timestamp` which is
    /// the message-author-set time on platforms that supply one).
    /// Connectors set both equal on platforms without a distinct
    /// server-received timestamp.
    #[serde(default)]
    #[ts(type = "number | null")]
    pub server_received_at_ms: Option<i64>,
    /// Structured author identity. `None` for legacy log entries.
    #[serde(default)]
    pub author: Option<ChatAuthor>,
    /// Backend-built renderable list. Empty for legacy log entries;
    /// the renderer then falls back to `message` text.
    #[serde(default)]
    pub fragments: Vec<MessageFragment>,
    /// Boolean attributes — emitted as a single integer on the wire
    /// (see [`MessageFlags`] serde impl).
    #[serde(default)]
    #[ts(type = "number")]
    pub flags: MessageFlags,
    /// Copy/search buffer; identical to `message` for inbound text.
    /// Optional so legacy entries don't carry a duplicate field.
    #[serde(default)]
    pub raw_text: Option<String>,
    /// Total bits in a Twitch cheer message. `None` if not a cheer.
    #[serde(default)]
    pub bits_total: Option<u32>,
    /// Optional row-tinting color (e.g. message highlighted by points
    /// reward, or platform-supplied background color).
    #[serde(default)]
    pub highlight_color: Option<FragmentColor>,
    /// Twitch Hype Chat pin-duration tier when this message is an
    /// elevated pin.
    #[serde(default)]
    pub elevated_tier: Option<ElevatedTier>,
    /// Reply-thread anchor when this message is a reply.
    #[serde(default)]
    pub reply: Option<ReplyContext>,
    /// Non-text event payload. Either set (a system event) or text
    /// (`fragments` non-empty); a single message never carries both.
    #[serde(default)]
    pub event: Option<ChatEvent>,
    /// Message direction (inbound/outbound)
    #[serde(default)]
    pub direction: ChatMessageDirection,
    /// Source message ID from the platform (if available)
    pub source_id: Option<String>,
    /// Legacy: user's display color (hex string). New code reads
    /// `author.color`.
    pub color: Option<String>,
    /// Legacy: raw badge codes. New code reads `author.badges_raw` (or
    /// the resolved `Badge` fragments in `fragments`).
    pub badges: Option<Vec<String>>,
}

impl ChatMessage {
    pub fn new(platform: ChatPlatform, username: String, message: String) -> Self {
        let now_ms = chrono::Utc::now().timestamp_millis();
        Self {
            id: Uuid::new_v4().to_string(),
            platform,
            account_id: None,
            channel_id: None,
            platforms: None,
            username,
            message,
            timestamp: now_ms,
            server_received_at_ms: Some(now_ms),
            author: None,
            fragments: Vec::new(),
            flags: MessageFlags::empty(),
            raw_text: None,
            bits_total: None,
            highlight_color: None,
            elevated_tier: None,
            reply: None,
            event: None,
            direction: ChatMessageDirection::Inbound,
            source_id: None,
            color: None,
            badges: None,
        }
    }

    pub fn new_outbound(platforms: Vec<ChatPlatform>, username: String, message: String) -> Self {
        let primary = platforms.first().copied().unwrap_or(ChatPlatform::Twitch);
        let now_ms = chrono::Utc::now().timestamp_millis();
        Self {
            id: Uuid::new_v4().to_string(),
            platform: primary,
            account_id: None,
            channel_id: None,
            platforms: Some(platforms),
            username,
            message,
            timestamp: now_ms,
            server_received_at_ms: Some(now_ms),
            author: None,
            fragments: Vec::new(),
            flags: MessageFlags::empty(),
            raw_text: None,
            bits_total: None,
            highlight_color: None,
            elevated_tier: None,
            reply: None,
            event: None,
            direction: ChatMessageDirection::Outbound,
            source_id: None,
            color: None,
            badges: None,
        }
    }

    pub fn with_color(mut self, color: String) -> Self {
        self.color = Some(color);
        self
    }

    pub fn with_source_id(mut self, source_id: String) -> Self {
        self.id = format!("{}:{}", self.platform.as_str(), source_id);
        self.source_id = Some(source_id);
        self
    }

    pub fn with_badges(mut self, badges: Vec<String>) -> Self {
        self.badges = Some(badges);
        self
    }
}

// ---------------------------------------------------------------------------
// Phase B: structured-fragment chat model
//
// The new pipeline pre-parses platform-specific payloads into a tagged-union
// `MessageFragment` list and a `MessageFlags` bitfield. Frontend renders the
// list verbatim and dispatches on `fragment.kind`; never parses raw text.
// Backend is the only place that knows IRC tags, Helix metadata, YouTube
// liveChatMessages shapes, 7TV/BTTV/FFZ token tables, etc.
// ---------------------------------------------------------------------------

/// Validated CSS-safe colour. `hex` is a 7-char `"#RRGGBB"` string;
/// constructed via [`FragmentColor::from_hex`] so user-supplied IRC
/// `color=` tags (which can be malformed) never reach the renderer
/// without validation.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct FragmentColor {
    pub hex: String,
}

impl FragmentColor {
    /// Accept either `#RRGGBB` or `RRGGBB` (with optional leading `#`),
    /// case-insensitive. Returns `None` for anything else — the
    /// renderer falls back to the platform-default colour.
    pub fn from_hex(raw: &str) -> Option<Self> {
        let trimmed = raw.trim().trim_start_matches('#');
        if trimmed.len() != 6 || !trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        Some(Self {
            hex: format!("#{}", trimmed.to_uppercase()),
        })
    }
}

/// Origin of an [`MessageFragment::Emote`]. Drives the resolved-URL
/// pattern in the connector layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub enum EmoteProvider {
    Twitch,
    Bttv,
    Ffz,
    SevenTv,
    Emoji,
    Kick,
}

/// Origin of an [`MessageFragment::Badge`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub enum BadgeProvider {
    Twitch,
    Ffz,
    SevenTv,
    Chatterino,
    Site,
}

/// Optional text styling on a [`MessageFragment::Text`] run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, TS)]
#[serde(rename_all = "PascalCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub enum TextStyle {
    #[default]
    Normal,
    Bold,
    Italic,
    Monospace,
}

/// One renderable unit of a chat message. The frontend dispatches on
/// `kind` — every parsing decision (emote resolution, mention detection,
/// link extraction, bits prefix matching) happens before this leaves
/// core.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub enum MessageFragment {
    #[serde(rename_all = "camelCase")]
    Text {
        content: String,
        color: Option<FragmentColor>,
        #[serde(default)]
        style: TextStyle,
    },
    #[serde(rename_all = "camelCase")]
    Mention {
        login: String,
        display_name: String,
        user_color: Option<FragmentColor>,
    },
    #[serde(rename_all = "camelCase")]
    Link {
        url: String,
        display: String,
        is_safe_browsing_flagged: bool,
    },
    #[serde(rename_all = "camelCase")]
    Emote {
        provider: EmoteProvider,
        id: String,
        name: String,
        animated: bool,
        zero_width: bool,
        url_1x: String,
        url_2x: String,
        url_4x: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    LayeredEmote {
        base: Box<MessageFragment>,
        overlays: Vec<MessageFragment>,
    },
    #[serde(rename_all = "camelCase")]
    Badge {
        provider: BadgeProvider,
        id: String,
        title: String,
        url_1x: String,
        url_2x: String,
        tint: Option<FragmentColor>,
    },
    #[serde(rename_all = "camelCase")]
    Cheermote {
        prefix: String,
        amount: u32,
        tier_color: FragmentColor,
        url_1x: String,
        url_2x: String,
    },
    #[serde(rename_all = "camelCase")]
    Timestamp {
        #[ts(type = "number")]
        unix_ms: i64,
        formatted: String,
    },
    #[serde(rename_all = "camelCase")]
    ReplyPreview {
        parent_message_id: String,
        parent_login: String,
        parent_display_name: String,
        parent_text_preview: String,
    },
    Linebreak,
}

/// Author metadata for a chat message. Resolved badges live in
/// `ChatMessage::fragments` as `Badge` fragments; `badges_raw` carries
/// the platform-side codes so consumers that don't render badges can
/// still filter on them (e.g. moderator-only views).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct ChatAuthor {
    pub user_id: String,
    pub login: String,
    pub display_name: String,
    pub color: Option<FragmentColor>,
    #[serde(default)]
    pub badges_raw: Vec<String>,
}

bitflags::bitflags! {
    /// Boolean attributes attached to a [`ChatMessage`]. Stored as a
    /// single `u64` on the wire (ts-rs export is pinned to `number`)
    /// so the frontend bit-checks via `(flags & MASK) !== 0`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct MessageFlags: u64 {
        const HIGHLIGHTED                     = 1 << 0;
        const FIRST_MESSAGE                   = 1 << 1;
        const ELEVATED_MESSAGE                = 1 << 2;
        const CHEER_MESSAGE                   = 1 << 3;
        const REPLY_MESSAGE                   = 1 << 4;
        const ACTION                          = 1 << 5;
        const SYSTEM                          = 1 << 6;
        const SUBSCRIPTION                    = 1 << 7;
        const ANNOUNCEMENT                    = 1 << 8;
        const WHISPER                         = 1 << 9;
        const DISABLED                        = 1 << 10;
        const TIMED_OUT_AUTHOR                = 1 << 11;
        const AUTOMOD_HELD                    = 1 << 12;
        const RESTRICTED_AUTHOR               = 1 << 13;
        const MONITORED_AUTHOR                = 1 << 14;
        const SHARED_FROM_OTHER_CHANNEL       = 1 << 15;
        const REDEEMED_CHANNEL_POINT_REWARD   = 1 << 16;
    }
}

// Serialize as the raw `u64` bit value rather than bitflags' default
// stringified form, so the wire shape matches what `#[ts(type =
// "number")]` declares. Frontend reads a plain number.
impl Serialize for MessageFlags {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.bits().serialize(s)
    }
}

impl<'de> Deserialize<'de> for MessageFlags {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = u64::deserialize(d)?;
        Ok(MessageFlags::from_bits_truncate(raw))
    }
}

/// Twitch Hype Chat pin-duration tier. Mapped from the
/// `pinned-chat-paid-level` IRC tag value (`ONE_MINUTE` … `FIVE_HOURS`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub enum ElevatedTier {
    OneMin,
    FiveMin,
    TenMin,
    ThirtyMin,
    OneHour,
    TwoHour,
    ThreeHour,
    FourHour,
    FiveHour,
}

/// Reply-thread anchor for a [`ChatMessage`]. The first-fragment
/// `ReplyPreview` carries the rendered preview; this struct lets the
/// frontend group messages into threads without re-parsing fragments.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct ReplyContext {
    pub parent_message_id: String,
    pub thread_root_id: String,
}

// --- ChatEvent payloads ----------------------------------------------------

/// YouTube super-chat payment. `amount_micros` is the integer
/// micro-unit value YouTube returns (e.g. `2500000` for $2.50 USD).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct SuperChatPayload {
    #[ts(type = "number")]
    pub amount_micros: i64,
    pub currency: String,
    pub tier: u8,
    pub message: String,
    pub background_color: Option<FragmentColor>,
}

/// YouTube super-sticker. Same money shape as super-chat plus a
/// platform sticker ID; the URL is resolved by the connector.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct SuperStickerPayload {
    #[ts(type = "number")]
    pub amount_micros: i64,
    pub currency: String,
    pub tier: u8,
    pub sticker_id: String,
    pub sticker_url: String,
    pub alt_text: String,
}

/// Twitch Hype Chat — paid pinned message. Tier maps to
/// [`ElevatedTier`]; amount is the micro-unit currency value.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct HypeChatPayload {
    #[ts(type = "number")]
    pub amount_micros: i64,
    pub currency: String,
    pub tier: ElevatedTier,
}

/// YouTube membership-join event.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct NewSponsorPayload {
    pub sponsor_login: String,
    pub sponsor_display_name: String,
    pub tier_name: String,
}

/// Recurring-membership milestone (YouTube N-month badge).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct MilestonePayload {
    pub months: u32,
    pub display_name: String,
    pub message: String,
}

/// Sub-gift announcement. Recipient list is plural so a single mass-gift
/// event (Twitch `submysterygift`) is one record.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct GiftedPayload {
    pub gifter_login: String,
    pub gifter_display_name: String,
    pub count: u32,
    pub recipient_logins: Vec<String>,
    pub tier: String,
}

/// Bits cheer summary. The cheer fragments are already in
/// `ChatMessage::fragments`; this carries the totals.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct CheerPayload {
    pub bits: u32,
    pub user_total_bits: Option<u32>,
}

/// Incoming Twitch raid.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct RaidPayload {
    pub raider_login: String,
    pub raider_display_name: String,
    pub viewer_count: u32,
}

/// CLEARCHAT / liveChatBans.insert. `duration_secs == None` means
/// permanent ban; `Some(N)` is a timeout.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct BanPayload {
    pub user_id: String,
    pub user_login: String,
    pub duration_secs: Option<u32>,
    pub reason: Option<String>,
}

/// Poll announcement / result snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct PollPayload {
    pub poll_id: String,
    pub question: String,
    pub choices: Vec<String>,
    pub votes: Option<Vec<u32>>,
}

/// Twitch ROOMSTATE — channel-level state change (slow mode,
/// follower-only, sub-only, emote-only, r9k). All fields are
/// `Option`: Twitch sends one ROOMSTATE on join with every field set,
/// and incremental ROOMSTATEs with only the changed field populated.
/// `None` means "this setting was not updated by this message", not
/// "this setting is off".
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct RoomStatePayload {
    /// `Some(true)` = emote-only on; `Some(false)` = off; `None` = unchanged.
    pub emote_only: Option<bool>,
    /// `Some(true)` = subscribers-only on; `Some(false)` = off; `None` = unchanged.
    pub subscribers_only: Option<bool>,
    /// `Some(true)` = r9k (unique-chat) on; `Some(false)` = off; `None` = unchanged.
    pub r9k: Option<bool>,
    /// Slow-mode delay in seconds. `Some(0)` = slow mode off;
    /// `Some(N)` = N-second minimum between messages; `None` = unchanged.
    pub slow_mode_secs: Option<u32>,
    /// Followers-only delay in seconds. `Some(0)` = any follower may chat;
    /// `Some(N)` = follower for at least N minutes (Twitch reports minutes);
    /// `None` = mode unchanged. Use the dedicated `followers_only_disabled`
    /// flag to distinguish "off" from "any-follower".
    pub followers_only_minutes: Option<u32>,
    /// Disambiguates the followers-only field: `Some(true)` when the
    /// update explicitly disabled followers-only (separate from
    /// "no change" / "any follower / N-minute minimum").
    pub followers_only_disabled: Option<bool>,
}

/// Non-text chat events. Surfaced via [`ChatMessage::event`] so the
/// same stream carries plain messages and system events; consumers
/// match on `kind` to render system rows distinctly.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub enum ChatEvent {
    SuperChat(SuperChatPayload),
    SuperSticker(SuperStickerPayload),
    HypeChat(HypeChatPayload),
    NewSponsor(NewSponsorPayload),
    MemberMilestone(MilestonePayload),
    SubGifted(GiftedPayload),
    Cheer(CheerPayload),
    Raid(RaidPayload),
    #[serde(rename_all = "camelCase")]
    MessageDeleted {
        id: String,
    },
    UserBanned(BanPayload),
    ChatEnded,
    Poll(PollPayload),
    /// Channel-level state change (Twitch ROOMSTATE). Carries the
    /// delta — fields the update touched are `Some`, untouched fields
    /// are `None`. Frontend tracks the *current* aggregate state by
    /// folding each delta into a local cache.
    RoomStateChanged(RoomStatePayload),
    Tombstone,
}

/// Direction for chat messages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub enum ChatMessageDirection {
    #[default]
    Inbound,
    Outbound,
}

/// Configuration for a chat platform connection
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct ChatConfig {
    /// Platform to connect to
    pub platform: ChatPlatform,
    /// Whether this platform is enabled
    pub enabled: bool,
    /// Platform-specific configuration
    pub credentials: ChatCredentials,
}

/// Platform-specific credentials
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "lowercase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub enum ChatCredentials {
    #[serde(rename_all = "camelCase")]
    Twitch {
        /// Twitch channel name to join
        channel: String,
        /// Authentication method (optional - anonymous read-only if not provided)
        auth: Option<TwitchAuth>,
    },
    #[serde(rename_all = "camelCase")]
    TikTok {
        /// TikTok username to monitor
        username: String,
        /// Session cookies/token (may be needed for some unofficial APIs)
        session_token: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    YouTube {
        /// YouTube channel ID or handle (e.g., "UCxxxxxx" or "@channelname")
        /// The backend will automatically find the current live stream
        channel_id: String,
        /// Authentication method
        auth: YouTubeAuth,
    },
    #[serde(rename_all = "camelCase")]
    Trovo {
        /// Trovo channel ID (numeric user/channel ID)
        channel_id: String,
    },
    #[serde(rename_all = "camelCase")]
    Stripchat {
        /// Stripchat model username
        username: String,
    },
    #[serde(rename_all = "camelCase")]
    Kick {
        /// Kick channel name (case-insensitive — Kick normalises internally).
        channel: String,
        /// OAuth bearer for the user account doing the chatting. `None`
        /// means read-only (anonymous Pusher subscription works without
        /// auth); `Some(token)` enables send via `api.kick.com/public/v1/chat`.
        #[serde(default)]
        oauth_token: Option<String>,
        /// Kick broadcaster user id (Kick's REST POST /chat expects the
        /// numeric broadcaster id, NOT the username). The chat lifecycle
        /// fetches this once when activating the profile.
        #[serde(default)]
        broadcaster_user_id: Option<u64>,
    },
    #[serde(rename_all = "camelCase")]
    Facebook {
        /// Facebook Live video ID
        video_id: String,
        /// Facebook access token
        access_token: String,
    },
}

/// Twitch authentication options
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "method", rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub enum TwitchAuth {
    /// User-provided OAuth token (from twitchtokengenerator.com or similar)
    #[serde(rename_all = "camelCase")]
    UserToken {
        /// OAuth token (with or without "oauth:" prefix)
        oauth_token: String,
    },
    /// App OAuth - user authenticated via "Login with Twitch" flow
    #[serde(rename_all = "camelCase")]
    AppOAuth {
        /// Access token from OAuth flow
        #[serde(default)]
        access_token: String,
        /// Refresh token for renewal
        refresh_token: Option<String>,
        /// Token expiration timestamp (Unix epoch seconds, JSON `number`).
        #[ts(type = "number | null")]
        expires_at: Option<i64>,
    },
}

/// YouTube authentication options
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "method", rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub enum YouTubeAuth {
    /// User-provided API key (preferred - uses user's own quota)
    #[serde(rename_all = "camelCase")]
    ApiKey {
        /// Google API key with YouTube Data API enabled
        key: String,
    },
    /// App OAuth - user authenticated via "Login with Google" flow
    #[serde(rename_all = "camelCase")]
    AppOAuth {
        /// Access token from OAuth flow
        #[serde(default)]
        access_token: String,
        /// Refresh token for renewal
        refresh_token: Option<String>,
        /// Token expiration timestamp (Unix epoch seconds, JSON `number`).
        #[ts(type = "number | null")]
        expires_at: Option<i64>,
    },
}

/// Connection status for a chat platform
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub enum ChatConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

/// Status information for a chat platform
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct ChatPlatformStatus {
    pub platform: ChatPlatform,
    pub status: ChatConnectionStatus,
    #[ts(type = "number")]
    pub message_count: u64,
    pub error: Option<String>,
}

/// Result of sending a chat message to a platform.
///
/// failures carry a stable machine-readable `errorCode`
/// (`chat_platform_not_connected` / `chat_sending_disabled` /
/// `chat_message_length_exceeded` / `internal`) alongside the human-readable
/// `error` message so clients can branch on the kind without parsing strings.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/types/src/generated/")]
pub struct ChatSendResult {
    pub platform: ChatPlatform,
    pub success: bool,
    pub error: Option<String>,
    pub error_code: Option<String>,
}

#[cfg(test)]
mod fragment_wire_shape {
    use super::*;
    use serde_json::Value;

    /// The ts-rs export for tagged-union enums emits inner struct
    /// fields in their declared (snake_case) names by default; serde
    /// applies the enum-level `rename_all = "camelCase"` to BOTH the
    /// discriminator and to inner fields. This test pins the actual
    /// wire shape (the source of truth) — if a future ts-rs upgrade
    /// or a careless enum-level annotation change desyncs the two
    /// representations, this fails and forces a fix.
    #[test]
    fn mention_fragment_serializes_inner_fields_as_camelcase() {
        let frag = MessageFragment::Mention {
            login: "alice".into(),
            display_name: "Alice".into(),
            user_color: None,
        };
        let json: Value = serde_json::from_str(&serde_json::to_string(&frag).unwrap()).unwrap();
        assert_eq!(json["kind"], "mention");
        assert!(json.get("displayName").is_some(), "expected camelCase displayName: {json}");
        assert!(json.get("userColor").is_some(), "expected camelCase userColor: {json}");
    }

    #[test]
    fn emote_fragment_url_fields_serialize_camelcase() {
        let frag = MessageFragment::Emote {
            provider: EmoteProvider::Twitch,
            id: "25".into(),
            name: "Kappa".into(),
            animated: false,
            zero_width: false,
            url_1x: "https://example/1x".into(),
            url_2x: "https://example/2x".into(),
            url_4x: None,
        };
        let json: Value = serde_json::from_str(&serde_json::to_string(&frag).unwrap()).unwrap();
        assert!(json.get("url1x").is_some(), "expected url1x camelCase: {json}");
        assert!(json.get("zeroWidth").is_some(), "expected zeroWidth camelCase: {json}");
    }

    #[test]
    fn message_flags_serialize_as_integer_bits() {
        let flags = MessageFlags::FIRST_MESSAGE | MessageFlags::ELEVATED_MESSAGE;
        let raw = serde_json::to_string(&flags).unwrap();
        // 1 << 1 (FIRST_MESSAGE) | 1 << 2 (ELEVATED_MESSAGE) = 0b110 = 6
        assert_eq!(raw, "6");
    }

    #[test]
    fn message_flags_roundtrip_unknown_bits_truncate() {
        let raw = "9223372036854775807"; // u64 max half — bits not in the enum get truncated
        let flags: MessageFlags = serde_json::from_str(raw).unwrap();
        // The known bits we declared are preserved; nothing panics.
        assert!(flags.bits() != 0);
    }

    #[test]
    fn fragment_color_validates_and_normalizes_hex() {
        assert_eq!(FragmentColor::from_hex("ff6699").unwrap().hex, "#FF6699");
        assert_eq!(FragmentColor::from_hex("#aabbcc").unwrap().hex, "#AABBCC");
        assert!(FragmentColor::from_hex("xyz").is_none());
        assert!(FragmentColor::from_hex("#1234").is_none());
        assert!(FragmentColor::from_hex("").is_none());
    }
}
