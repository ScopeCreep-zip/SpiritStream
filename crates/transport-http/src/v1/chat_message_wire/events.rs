//! `ChatEvent` wire-mirror and its payload tree.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use spiritstream_core::models::{
    BanPayload, ChatEvent, CheerPayload, GiftedPayload, HypeChatPayload, MilestonePayload,
    NewSponsorPayload, PollPayload, RaidPayload, RoomStatePayload, SuperChatPayload,
    SuperStickerPayload,
};

use super::{ElevatedTierWire, FragmentColorWire};

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

