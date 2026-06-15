use std::time::Instant;

use crate::models::{ChatAuthor, ChatMessage, ChatPlatform as ChatPlatformEnum};

#[derive(Debug, Clone)]
pub(super) struct OutboundMessage {
    pub(super) text: String,
    pub(super) timestamp: Instant,
}

/// Parses one YouTube live-chat `items[]` entry into a [`ChatMessage`], or
/// `None` when the entry is not a text message or carries empty text. Pure —
/// the poll loop owns delivery, counting, and the self-echo dedup (which needs
/// the connector's `recent_outbound` window and own channel id).
pub(crate) fn parse_youtube_chat_item(item: &serde_json::Value) -> Option<ChatMessage> {
    let snippet = &item["snippet"];
    let author = &item["authorDetails"];

    if snippet["type"].as_str().unwrap_or("") != "textMessageEvent" {
        return None;
    }

    let message_text = snippet["textMessageDetails"]["messageText"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();
    if message_text.is_empty() {
        return None;
    }

    let username = author["displayName"]
        .as_str()
        .unwrap_or("Unknown")
        .to_string();
    // The channel id is YouTube's stable per-viewer identifier. Populating
    // `author` lets anonymous mode key the friendly label off it (so the
    // alias stays stable even if the viewer renames) and hash it as the
    // canonical correlation id — parity with Twitch. YouTube has no separate
    // "login", so the channel id serves as both id and login.
    let channel_id = author["channelId"].as_str().unwrap_or("").to_string();

    let mut badges = Vec::new();
    if author["isChatOwner"].as_bool().unwrap_or(false) {
        badges.push("owner".to_string());
    }
    if author["isChatModerator"].as_bool().unwrap_or(false) {
        badges.push("moderator".to_string());
    }
    if author["isChatSponsor"].as_bool().unwrap_or(false) {
        badges.push("member".to_string());
    }

    let mut chat_msg = ChatMessage::new(ChatPlatformEnum::YouTube, username.clone(), message_text)
        .with_author(ChatAuthor {
            user_id: channel_id.clone(),
            login: channel_id,
            display_name: username,
            color: None,
            badges_raw: badges.clone(),
        });
    if let Some(source_id) = item["id"].as_str() {
        chat_msg = chat_msg.with_source_id(source_id.to_string());
    }
    if !badges.is_empty() {
        chat_msg = chat_msg.with_badges(badges);
    }

    Some(chat_msg)
}
