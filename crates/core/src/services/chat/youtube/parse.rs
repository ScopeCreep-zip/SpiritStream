use std::time::Instant;

use crate::models::{ChatMessage, ChatPlatform as ChatPlatformEnum};

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

    let mut chat_msg = ChatMessage::new(ChatPlatformEnum::YouTube, username, message_text);
    if let Some(source_id) = item["id"].as_str() {
        chat_msg = chat_msg.with_source_id(source_id.to_string());
    }

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
    if !badges.is_empty() {
        chat_msg = chat_msg.with_badges(badges);
    }

    Some(chat_msg)
}
