//! Wire-mirror types for [`ChatMessage`] and its dependency tree —
//! utoipa is transport-only, so `ToSchema` lives on these mirrors
//! rather than the core types.
//!
//! G5: replaces the `Vec<serde_json::Value>` return on
//! `v1_chat_search_session_proxy` with the fully-typed message tree.
//! Wire shape stays byte-identical to the ts-rs export at
//! `@spiritstream/types/ChatMessage` and friends.
//!
//! Reuses [`crate::v1::ChatPlatformWire`] for the platform enum (already
//! defined in `v1/chat/wire.rs`). `MessageFlags` is a bitflags `u64` on
//! the core side and a plain `u64` on the wire — matches the existing
//! ts-rs export shape (`flags: number`).

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use spiritstream_core::models::{
    BadgeProvider, BanPayload, ChatAuthor, ChatEvent, ChatMessage, ChatMessageDirection,
    CheerPayload, ElevatedTier, EmoteProvider, FragmentColor, GiftedPayload, HypeChatPayload,
    MessageFragment, MilestonePayload, NewSponsorPayload, PollPayload, RaidPayload, ReplyContext,
    RoomStatePayload, SuperChatPayload, SuperStickerPayload, TextStyle,
};

use crate::v1::ChatPlatformWire;

// ---------------------------------------------------------------------------
// Atoms used inside MessageFragment / ChatAuthor / ChatEvent payloads
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FragmentColorWire {
    pub hex: String,
}

impl From<FragmentColor> for FragmentColorWire {
    fn from(v: FragmentColor) -> Self {
        Self { hex: v.hex }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum EmoteProviderWire {
    Twitch,
    Bttv,
    Ffz,
    SevenTv,
    Emoji,
    Kick,
}

impl From<EmoteProvider> for EmoteProviderWire {
    fn from(v: EmoteProvider) -> Self {
        match v {
            EmoteProvider::Twitch => Self::Twitch,
            EmoteProvider::Bttv => Self::Bttv,
            EmoteProvider::Ffz => Self::Ffz,
            EmoteProvider::SevenTv => Self::SevenTv,
            EmoteProvider::Emoji => Self::Emoji,
            EmoteProvider::Kick => Self::Kick,
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum BadgeProviderWire {
    Twitch,
    Ffz,
    SevenTv,
    Chatterino,
    Site,
}

impl From<BadgeProvider> for BadgeProviderWire {
    fn from(v: BadgeProvider) -> Self {
        match v {
            BadgeProvider::Twitch => Self::Twitch,
            BadgeProvider::Ffz => Self::Ffz,
            BadgeProvider::SevenTv => Self::SevenTv,
            BadgeProvider::Chatterino => Self::Chatterino,
            BadgeProvider::Site => Self::Site,
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "PascalCase")]
pub enum TextStyleWire {
    Normal,
    Bold,
    Italic,
    Monospace,
}

impl From<TextStyle> for TextStyleWire {
    fn from(v: TextStyle) -> Self {
        match v {
            TextStyle::Normal => Self::Normal,
            TextStyle::Bold => Self::Bold,
            TextStyle::Italic => Self::Italic,
            TextStyle::Monospace => Self::Monospace,
        }
    }
}

// ---------------------------------------------------------------------------
// MessageFragment — recursive (LayeredEmote has Box<MessageFragment>)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MessageFragmentWire {
    #[serde(rename_all = "camelCase")]
    Text {
        content: String,
        color: Option<FragmentColorWire>,
        style: TextStyleWire,
    },
    #[serde(rename_all = "camelCase")]
    Mention {
        login: String,
        display_name: String,
        user_color: Option<FragmentColorWire>,
    },
    #[serde(rename_all = "camelCase")]
    Link {
        url: String,
        display: String,
        is_safe_browsing_flagged: bool,
    },
    #[serde(rename_all = "camelCase")]
    Emote {
        provider: EmoteProviderWire,
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
        base: Box<MessageFragmentWire>,
        overlays: Vec<MessageFragmentWire>,
    },
    #[serde(rename_all = "camelCase")]
    Badge {
        provider: BadgeProviderWire,
        id: String,
        title: String,
        url_1x: String,
        url_2x: String,
        tint: Option<FragmentColorWire>,
    },
    #[serde(rename_all = "camelCase")]
    Cheermote {
        prefix: String,
        amount: u32,
        tier_color: FragmentColorWire,
        url_1x: String,
        url_2x: String,
    },
    #[serde(rename_all = "camelCase")]
    Timestamp {
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

impl From<MessageFragment> for MessageFragmentWire {
    fn from(v: MessageFragment) -> Self {
        match v {
            MessageFragment::Text {
                content,
                color,
                style,
            } => Self::Text {
                content,
                color: color.map(Into::into),
                style: style.into(),
            },
            MessageFragment::Mention {
                login,
                display_name,
                user_color,
            } => Self::Mention {
                login,
                display_name,
                user_color: user_color.map(Into::into),
            },
            MessageFragment::Link {
                url,
                display,
                is_safe_browsing_flagged,
            } => Self::Link {
                url,
                display,
                is_safe_browsing_flagged,
            },
            MessageFragment::Emote {
                provider,
                id,
                name,
                animated,
                zero_width,
                url_1x,
                url_2x,
                url_4x,
            } => Self::Emote {
                provider: provider.into(),
                id,
                name,
                animated,
                zero_width,
                url_1x,
                url_2x,
                url_4x,
            },
            MessageFragment::LayeredEmote { base, overlays } => Self::LayeredEmote {
                base: Box::new((*base).into()),
                overlays: overlays.into_iter().map(Into::into).collect(),
            },
            MessageFragment::Badge {
                provider,
                id,
                title,
                url_1x,
                url_2x,
                tint,
            } => Self::Badge {
                provider: provider.into(),
                id,
                title,
                url_1x,
                url_2x,
                tint: tint.map(Into::into),
            },
            MessageFragment::Cheermote {
                prefix,
                amount,
                tier_color,
                url_1x,
                url_2x,
            } => Self::Cheermote {
                prefix,
                amount,
                tier_color: tier_color.into(),
                url_1x,
                url_2x,
            },
            MessageFragment::Timestamp { unix_ms, formatted } => {
                Self::Timestamp { unix_ms, formatted }
            }
            MessageFragment::ReplyPreview {
                parent_message_id,
                parent_login,
                parent_display_name,
                parent_text_preview,
            } => Self::ReplyPreview {
                parent_message_id,
                parent_login,
                parent_display_name,
                parent_text_preview,
            },
            MessageFragment::Linebreak => Self::Linebreak,
        }
    }
}

// ---------------------------------------------------------------------------
// ChatAuthor / ReplyContext / ElevatedTier
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatAuthorWire {
    pub user_id: String,
    pub login: String,
    pub display_name: String,
    pub color: Option<FragmentColorWire>,
    pub badges_raw: Vec<String>,
}

impl From<ChatAuthor> for ChatAuthorWire {
    fn from(v: ChatAuthor) -> Self {
        Self {
            user_id: v.user_id,
            login: v.login,
            display_name: v.display_name,
            color: v.color.map(Into::into),
            badges_raw: v.badges_raw,
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ElevatedTierWire {
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

impl From<ElevatedTier> for ElevatedTierWire {
    fn from(v: ElevatedTier) -> Self {
        match v {
            ElevatedTier::OneMin => Self::OneMin,
            ElevatedTier::FiveMin => Self::FiveMin,
            ElevatedTier::TenMin => Self::TenMin,
            ElevatedTier::ThirtyMin => Self::ThirtyMin,
            ElevatedTier::OneHour => Self::OneHour,
            ElevatedTier::TwoHour => Self::TwoHour,
            ElevatedTier::ThreeHour => Self::ThreeHour,
            ElevatedTier::FourHour => Self::FourHour,
            ElevatedTier::FiveHour => Self::FiveHour,
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReplyContextWire {
    pub parent_message_id: String,
    pub thread_root_id: String,
}

impl From<ReplyContext> for ReplyContextWire {
    fn from(v: ReplyContext) -> Self {
        Self {
            parent_message_id: v.parent_message_id,
            thread_root_id: v.thread_root_id,
        }
    }
}

// ---------------------------------------------------------------------------
// ChatEvent payload structs
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SuperChatPayloadWire {
    pub amount_micros: i64,
    pub currency: String,
    pub tier: u8,
    pub message: String,
    pub background_color: Option<FragmentColorWire>,
}

impl From<SuperChatPayload> for SuperChatPayloadWire {
    fn from(v: SuperChatPayload) -> Self {
        Self {
            amount_micros: v.amount_micros,
            currency: v.currency,
            tier: v.tier,
            message: v.message,
            background_color: v.background_color.map(Into::into),
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SuperStickerPayloadWire {
    pub amount_micros: i64,
    pub currency: String,
    pub tier: u8,
    pub sticker_id: String,
    pub sticker_url: String,
    pub alt_text: String,
}

impl From<SuperStickerPayload> for SuperStickerPayloadWire {
    fn from(v: SuperStickerPayload) -> Self {
        Self {
            amount_micros: v.amount_micros,
            currency: v.currency,
            tier: v.tier,
            sticker_id: v.sticker_id,
            sticker_url: v.sticker_url,
            alt_text: v.alt_text,
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HypeChatPayloadWire {
    pub amount_micros: i64,
    pub currency: String,
    pub tier: ElevatedTierWire,
}

impl From<HypeChatPayload> for HypeChatPayloadWire {
    fn from(v: HypeChatPayload) -> Self {
        Self {
            amount_micros: v.amount_micros,
            currency: v.currency,
            tier: v.tier.into(),
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewSponsorPayloadWire {
    pub sponsor_login: String,
    pub sponsor_display_name: String,
    pub tier_name: String,
}

impl From<NewSponsorPayload> for NewSponsorPayloadWire {
    fn from(v: NewSponsorPayload) -> Self {
        Self {
            sponsor_login: v.sponsor_login,
            sponsor_display_name: v.sponsor_display_name,
            tier_name: v.tier_name,
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MilestonePayloadWire {
    pub months: u32,
    pub display_name: String,
    pub message: String,
}

impl From<MilestonePayload> for MilestonePayloadWire {
    fn from(v: MilestonePayload) -> Self {
        Self {
            months: v.months,
            display_name: v.display_name,
            message: v.message,
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GiftedPayloadWire {
    pub gifter_login: String,
    pub gifter_display_name: String,
    pub count: u32,
    pub recipient_logins: Vec<String>,
    pub tier: String,
}

impl From<GiftedPayload> for GiftedPayloadWire {
    fn from(v: GiftedPayload) -> Self {
        Self {
            gifter_login: v.gifter_login,
            gifter_display_name: v.gifter_display_name,
            count: v.count,
            recipient_logins: v.recipient_logins,
            tier: v.tier,
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CheerPayloadWire {
    pub bits: u32,
    pub user_total_bits: Option<u32>,
}

impl From<CheerPayload> for CheerPayloadWire {
    fn from(v: CheerPayload) -> Self {
        Self {
            bits: v.bits,
            user_total_bits: v.user_total_bits,
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RaidPayloadWire {
    pub raider_login: String,
    pub raider_display_name: String,
    pub viewer_count: u32,
}

impl From<RaidPayload> for RaidPayloadWire {
    fn from(v: RaidPayload) -> Self {
        Self {
            raider_login: v.raider_login,
            raider_display_name: v.raider_display_name,
            viewer_count: v.viewer_count,
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BanPayloadWire {
    pub user_id: String,
    pub user_login: String,
    pub duration_secs: Option<u32>,
    pub reason: Option<String>,
}

impl From<BanPayload> for BanPayloadWire {
    fn from(v: BanPayload) -> Self {
        Self {
            user_id: v.user_id,
            user_login: v.user_login,
            duration_secs: v.duration_secs,
            reason: v.reason,
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PollPayloadWire {
    pub poll_id: String,
    pub question: String,
    pub choices: Vec<String>,
    pub votes: Option<Vec<u32>>,
}

impl From<PollPayload> for PollPayloadWire {
    fn from(v: PollPayload) -> Self {
        Self {
            poll_id: v.poll_id,
            question: v.question,
            choices: v.choices,
            votes: v.votes,
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoomStatePayloadWire {
    pub emote_only: Option<bool>,
    pub subscribers_only: Option<bool>,
    pub r9k: Option<bool>,
    pub slow_mode_secs: Option<u32>,
    pub followers_only_minutes: Option<u32>,
    pub followers_only_disabled: Option<bool>,
}

impl From<RoomStatePayload> for RoomStatePayloadWire {
    fn from(v: RoomStatePayload) -> Self {
        Self {
            emote_only: v.emote_only,
            subscribers_only: v.subscribers_only,
            r9k: v.r9k,
            slow_mode_secs: v.slow_mode_secs,
            followers_only_minutes: v.followers_only_minutes,
            followers_only_disabled: v.followers_only_disabled,
        }
    }
}

// ---------------------------------------------------------------------------
// ChatEvent (tagged "kind") — variants wrap the payload structs above
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ChatEventWire {
    SuperChat(SuperChatPayloadWire),
    SuperSticker(SuperStickerPayloadWire),
    HypeChat(HypeChatPayloadWire),
    NewSponsor(NewSponsorPayloadWire),
    MemberMilestone(MilestonePayloadWire),
    SubGifted(GiftedPayloadWire),
    Cheer(CheerPayloadWire),
    Raid(RaidPayloadWire),
    #[serde(rename_all = "camelCase")]
    MessageDeleted {
        id: String,
    },
    UserBanned(BanPayloadWire),
    ChatEnded,
    Poll(PollPayloadWire),
    RoomStateChanged(RoomStatePayloadWire),
    Tombstone,
}

impl From<ChatEvent> for ChatEventWire {
    fn from(v: ChatEvent) -> Self {
        match v {
            ChatEvent::SuperChat(p) => Self::SuperChat(p.into()),
            ChatEvent::SuperSticker(p) => Self::SuperSticker(p.into()),
            ChatEvent::HypeChat(p) => Self::HypeChat(p.into()),
            ChatEvent::NewSponsor(p) => Self::NewSponsor(p.into()),
            ChatEvent::MemberMilestone(p) => Self::MemberMilestone(p.into()),
            ChatEvent::SubGifted(p) => Self::SubGifted(p.into()),
            ChatEvent::Cheer(p) => Self::Cheer(p.into()),
            ChatEvent::Raid(p) => Self::Raid(p.into()),
            ChatEvent::MessageDeleted { id } => Self::MessageDeleted { id },
            ChatEvent::UserBanned(p) => Self::UserBanned(p.into()),
            ChatEvent::ChatEnded => Self::ChatEnded,
            ChatEvent::Poll(p) => Self::Poll(p.into()),
            ChatEvent::RoomStateChanged(p) => Self::RoomStateChanged(p.into()),
            ChatEvent::Tombstone => Self::Tombstone,
        }
    }
}

// ---------------------------------------------------------------------------
// ChatMessageDirection
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ChatMessageDirectionWire {
    Inbound,
    Outbound,
}

impl From<ChatMessageDirection> for ChatMessageDirectionWire {
    fn from(v: ChatMessageDirection) -> Self {
        match v {
            ChatMessageDirection::Inbound => Self::Inbound,
            ChatMessageDirection::Outbound => Self::Outbound,
        }
    }
}

// ---------------------------------------------------------------------------
// ChatMessage (top-level)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageWire {
    pub id: String,
    pub platform: ChatPlatformWire,
    pub account_id: Option<String>,
    pub channel_id: Option<String>,
    pub platforms: Option<Vec<ChatPlatformWire>>,
    pub username: String,
    pub message: String,
    pub timestamp: i64,
    pub server_received_at_ms: Option<i64>,
    pub author: Option<ChatAuthorWire>,
    pub fragments: Vec<MessageFragmentWire>,
    /// `MessageFlags` bitflag serialised as a plain u64. Wire shape
    /// matches the ts-rs export (`flags: number`); frontend does
    /// `(flags & MASK) !== 0` to test individual bits.
    pub flags: u64,
    pub raw_text: Option<String>,
    pub bits_total: Option<u32>,
    pub highlight_color: Option<FragmentColorWire>,
    pub elevated_tier: Option<ElevatedTierWire>,
    pub reply: Option<ReplyContextWire>,
    pub event: Option<ChatEventWire>,
    pub direction: ChatMessageDirectionWire,
    pub source_id: Option<String>,
    pub color: Option<String>,
    pub badges: Option<Vec<String>>,
}

impl From<ChatMessage> for ChatMessageWire {
    fn from(v: ChatMessage) -> Self {
        Self {
            id: v.id,
            platform: v.platform.into(),
            account_id: v.account_id,
            channel_id: v.channel_id,
            platforms: v
                .platforms
                .map(|ps| ps.into_iter().map(Into::into).collect()),
            username: v.username,
            message: v.message,
            timestamp: v.timestamp,
            server_received_at_ms: v.server_received_at_ms,
            author: v.author.map(Into::into),
            fragments: v.fragments.into_iter().map(Into::into).collect(),
            flags: v.flags.bits(),
            raw_text: v.raw_text,
            bits_total: v.bits_total,
            highlight_color: v.highlight_color.map(Into::into),
            elevated_tier: v.elevated_tier.map(Into::into),
            reply: v.reply.map(Into::into),
            event: v.event.map(Into::into),
            direction: v.direction.into(),
            source_id: v.source_id,
            color: v.color,
            badges: v.badges,
        }
    }
}
