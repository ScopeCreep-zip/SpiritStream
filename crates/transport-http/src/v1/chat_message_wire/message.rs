//! Top-level `ChatMessage` wire-mirror + direction enum.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use spiritstream_core::models::{ChatMessage, ChatMessageDirection};

use crate::v1::ChatPlatformWire;

use super::{
    ChatAuthorWire, ChatEventWire, ElevatedTierWire, FragmentColorWire, MessageFragmentWire,
    ReplyContextWire,
};

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
