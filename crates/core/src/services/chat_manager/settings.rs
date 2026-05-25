use std::sync::atomic::Ordering;

use crate::models::{ChatPlatform, ChatSettings};

impl super::ChatManager {
    /// Update the cached chat settings for the active profile.
    pub async fn update_profile_chat_settings(&self, settings: ChatSettings) {
        {
            let mut guard = self.chat_settings.lock().await;
            *guard = settings.clone();
        }

        self.set_crosspost_enabled(settings.crosspost_enabled);
        self.set_send_enabled(ChatPlatform::Twitch, settings.twitch_send_enabled)
            .await;
        self.set_send_enabled(ChatPlatform::YouTube, settings.youtube_send_enabled)
            .await;
        self.set_send_enabled(ChatPlatform::Trovo, settings.trovo_send_enabled)
            .await;
        self.set_send_enabled(ChatPlatform::Stripchat, settings.stripchat_send_enabled)
            .await;
    }

    /// Get the cached chat settings for the active profile.
    pub async fn profile_chat_settings(&self) -> ChatSettings {
        self.chat_settings.lock().await.clone()
    }

    /// Enable or disable crossposting of inbound messages.
    pub fn set_crosspost_enabled(&self, enabled: bool) {
        self.crosspost_enabled.store(enabled, Ordering::Relaxed);
    }

    /// Update per-platform send enable flags for crossposting.
    pub async fn set_send_enabled(&self, platform: ChatPlatform, enabled: bool) {
        let mut map = self.send_enabled.lock().await;
        map.insert(platform, enabled);
    }

    /// Push the active profile's anonymous-mode policy.
    /// Transports call this on profile activate so subsequent chat
    /// messages logged through `log_message` get pseudonymised transparently.
    pub async fn set_anonymous_policy(&self, enabled: bool, salt_hex: String) {
        let mut guard = self.anonymous_policy.lock().await;
        *guard = Some((enabled, salt_hex));
    }

    /// Clear the active anonymous-mode policy. Called when
    /// the user deactivates a profile or revokes the policy outright.
    pub async fn clear_anonymous_policy(&self) {
        let mut guard = self.anonymous_policy.lock().await;
        *guard = None;
    }
}
