//! `MessageFragment` wire-mirror.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use spiritstream_core::models::MessageFragment;

use super::{BadgeProviderWire, EmoteProviderWire, FragmentColorWire, TextStyleWire};

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

