//! Atom wire-mirrors used inside `MessageFragment` / `ChatAuthor` /
//! `ChatEvent` payloads.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use spiritstream_core::models::{BadgeProvider, EmoteProvider, FragmentColor, TextStyle};

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

