//! `ChatAuthor` / `ElevatedTier` / `ReplyContext` wire-mirrors.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use spiritstream_core::models::{ChatAuthor, ElevatedTier, ReplyContext};

use super::FragmentColorWire;

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
