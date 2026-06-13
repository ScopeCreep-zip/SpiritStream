use std::sync::atomic::Ordering;

use crate::models::{ChatConnectionStatus, ChatPlatform, ChatSettings};

impl super::ChatManager {
    /// Update the cached chat settings for the active profile.
    pub async fn update_profile_chat_settings(&self, settings: ChatSettings) {
        let previous = {
            let mut guard = self.chat_settings.lock().await;
            let prev = guard.clone();
            *guard = settings.clone();
            prev
        };

        // A changed channel for a currently-connected platform means the
        // live connection is now joined to the wrong channel. Disconnect
        // it (clearing intent) so the activation auto-connect rebuilds it
        // on the new channel — the already-connected check would
        // otherwise leave it stuck on the stale channel forever (harmless
        // while chat was stream-gated; a real bug now that chat is
        // always-on).
        self.reconnect_changed_channels(&previous, &settings).await;

        self.set_crosspost_enabled(settings.crosspost_enabled);
        self.set_send_enabled(ChatPlatform::Twitch, settings.twitch_send_enabled)
            .await;
        self.set_send_enabled(ChatPlatform::YouTube, settings.youtube_send_enabled)
            .await;
        self.set_send_enabled(ChatPlatform::Trovo, settings.trovo_send_enabled)
            .await;
    }

    /// Disconnect any connected platform whose channel/username changed,
    /// clearing its disconnect-intent so the caller's auto-connect can
    /// rebuild it on the new target.
    async fn reconnect_changed_channels(&self, prev: &ChatSettings, next: &ChatSettings) {
        let changed = [
            (ChatPlatform::Twitch, &prev.twitch_channel, &next.twitch_channel),
            (
                ChatPlatform::YouTube,
                &prev.youtube_channel_id,
                &next.youtube_channel_id,
            ),
            (
                ChatPlatform::Trovo,
                &prev.trovo_channel_id,
                &next.trovo_channel_id,
            ),
            (ChatPlatform::Kick, &prev.kick_channel, &next.kick_channel),
            (
                ChatPlatform::TikTok,
                &prev.tiktok_username,
                &next.tiktok_username,
            ),
        ];
        for (platform, before, after) in changed {
            if before == after {
                continue;
            }
            let connected = self
                .get_platform_status(platform)
                .await
                .is_some_and(|s| s.status == ChatConnectionStatus::Connected);
            if connected {
                if let Err(e) = self.disconnect(platform).await {
                    log::warn!(
                        "failed to drop {} on channel change: {e}",
                        platform.as_str()
                    );
                }
                // disconnect() set the intent; the user changing a
                // channel implies they WANT this platform, so clear it
                // and let auto-connect re-establish on the new channel.
                self.clear_disconnect_intent(platform).await;
            }
        }
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
    ///
    /// G2: emits `AnonymousModeToggled` to the audit chain when the
    /// effective `enabled` flag changes from the previous policy (or no
    /// previous policy → enabled = true). The state change is the
    /// audit-relevant event, not every profile-activation that re-pushes
    /// the same policy.
    ///
    /// Fails with [`crate::CoreError::AnonymousSaltInvalid`] when
    /// `enabled` is true but the salt is empty or malformed: an enabled
    /// policy with a broken salt would otherwise force every inbound
    /// message to be dropped (the pseudonymizer fails loud), so the
    /// activation that pushes the policy must fail instead.
    pub fn set_anonymous_policy(
        &self,
        enabled: bool,
        salt_hex: String,
    ) -> Result<(), crate::CoreError> {
        if enabled {
            // Probe the salt once at policy-set time so a bad salt is an
            // activation error, not a per-message drop storm.
            crate::services::pseudonymizer::pseudonymize("probe", &salt_hex)?;
        }
        let mut guard = self
            .anonymous_policy
            .write()
            .unwrap_or_else(|e| e.into_inner());
        let prev_enabled = guard.as_ref().map(|(en, _)| *en);
        let changed = prev_enabled != Some(enabled);
        *guard = Some((enabled, salt_hex));
        drop(guard);
        if changed {
            if let Some(audit) = self.audit() {
                if let Err(e) =
                    audit.record(crate::services::AuditAction::AnonymousModeToggled { enabled })
                {
                    log::error!(
                        "chat_manager failed to append AnonymousModeToggled audit entry: {e}"
                    );
                }
            }
        }
        Ok(())
    }

    /// Clear the active anonymous-mode policy. Called when
    /// the user deactivates a profile or revokes the policy outright.
    /// Treated as a transition to `enabled = false` for audit purposes.
    pub fn clear_anonymous_policy(&self) {
        let was_enabled = {
            let mut guard = self
                .anonymous_policy
                .write()
                .unwrap_or_else(|e| e.into_inner());
            let prev = guard.as_ref().map(|(en, _)| *en).unwrap_or(false);
            *guard = None;
            prev
        };
        if was_enabled {
            if let Some(audit) = self.audit() {
                if let Err(e) = audit
                    .record(crate::services::AuditAction::AnonymousModeToggled { enabled: false })
                {
                    log::error!(
                        "chat_manager failed to append AnonymousModeToggled audit entry: {e}"
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::chat_manager::ChatManager;
    use std::sync::Arc;

    struct NoopSink;
    impl crate::services::EventSink for NoopSink {
        fn emit(&self, _event: &str, _payload: serde_json::Value) {}
    }

    fn manager() -> (ChatManager, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let mgr = ChatManager::new(Arc::new(NoopSink), dir.path().to_path_buf());
        (mgr, dir)
    }

    #[tokio::test]
    async fn set_crosspost_enabled_toggles_flag() {
        let (mgr, _dir) = manager();
        mgr.set_crosspost_enabled(true);
        assert!(mgr.crosspost_enabled.load(Ordering::Relaxed));
        mgr.set_crosspost_enabled(false);
        assert!(!mgr.crosspost_enabled.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn set_send_enabled_records_per_platform() {
        let (mgr, _dir) = manager();
        mgr.set_send_enabled(ChatPlatform::Twitch, true).await;
        mgr.set_send_enabled(ChatPlatform::YouTube, false).await;
        let map = mgr.send_enabled.lock().await;
        assert_eq!(map.get(&ChatPlatform::Twitch), Some(&true));
        assert_eq!(map.get(&ChatPlatform::YouTube), Some(&false));
    }

    #[tokio::test]
    async fn update_profile_chat_settings_propagates_flags() {
        let (mgr, _dir) = manager();
        let settings = ChatSettings {
            crosspost_enabled: true,
            twitch_send_enabled: true,
            youtube_send_enabled: false,
            trovo_send_enabled: true,
            ..ChatSettings::default()
        };
        mgr.update_profile_chat_settings(settings).await;

        let cached = mgr.profile_chat_settings().await;
        assert!(cached.crosspost_enabled);
        assert!(cached.twitch_send_enabled);
        assert!(!cached.youtube_send_enabled);
        assert!(cached.trovo_send_enabled);
        assert!(mgr.crosspost_enabled.load(Ordering::Relaxed));
        let map = mgr.send_enabled.lock().await;
        assert_eq!(map.get(&ChatPlatform::Twitch), Some(&true));
        assert_eq!(map.get(&ChatPlatform::YouTube), Some(&false));
        assert_eq!(map.get(&ChatPlatform::Trovo), Some(&true));
    }

    #[tokio::test]
    async fn set_then_clear_anonymous_policy_round_trips() {
        let (mgr, _dir) = manager();
        mgr.set_anonymous_policy(true, "deadbeef".into()).unwrap();
        {
            let guard = mgr.anonymous_policy.read().unwrap();
            assert_eq!(guard.as_ref(), Some(&(true, "deadbeef".to_string())));
        }
        mgr.clear_anonymous_policy();
        assert!(mgr.anonymous_policy.read().unwrap().is_none());
    }

    /// Enabling anonymous mode with an empty or malformed salt must fail
    /// at policy-set time (i.e. fail the activation) instead of leaving
    /// an enabled-but-broken policy that drops every message.
    #[tokio::test]
    async fn set_anonymous_policy_rejects_invalid_salt_when_enabled() {
        let (mgr, _dir) = manager();
        assert!(matches!(
            mgr.set_anonymous_policy(true, String::new()),
            Err(crate::CoreError::AnonymousSaltInvalid)
        ));
        assert!(matches!(
            mgr.set_anonymous_policy(true, "not-hex".into()),
            Err(crate::CoreError::AnonymousSaltInvalid)
        ));
        // Disabled policy doesn't need a salt.
        mgr.set_anonymous_policy(false, String::new()).unwrap();
    }

    // ---- manual-disconnect intent (the decouple-from-streaming safety spine) ----

    #[tokio::test]
    async fn fresh_manager_has_no_disconnect_intent() {
        let (mgr, _dir) = manager();
        assert!(!mgr.is_disconnect_intended(ChatPlatform::Twitch).await);
    }

    /// The per-platform Disconnect button must record intent so the
    /// auto-connect / reconnect loops leave the platform down. `disconnect`
    /// sets intent BEFORE checking the connection (so an unconnected
    /// platform still gets the flag), then returns NotConnected — we only
    /// care about the intent side effect here.
    #[tokio::test]
    async fn disconnect_records_intent_even_when_not_connected() {
        let (mgr, _dir) = manager();
        let _ = mgr.disconnect(ChatPlatform::Twitch).await;
        assert!(mgr.is_disconnect_intended(ChatPlatform::Twitch).await);
        // Other platforms are untouched.
        assert!(!mgr.is_disconnect_intended(ChatPlatform::Kick).await);
    }

    /// An explicit reconnect (Connect button / sign-in) clears intent so
    /// the platform is free to stay connected again.
    #[tokio::test]
    async fn clear_intent_per_platform_and_all() {
        let (mgr, _dir) = manager();
        let _ = mgr.disconnect(ChatPlatform::Twitch).await;
        let _ = mgr.disconnect(ChatPlatform::Kick).await;
        assert!(mgr.is_disconnect_intended(ChatPlatform::Twitch).await);

        mgr.clear_disconnect_intent(ChatPlatform::Twitch).await;
        assert!(!mgr.is_disconnect_intended(ChatPlatform::Twitch).await);
        assert!(mgr.is_disconnect_intended(ChatPlatform::Kick).await);

        // Profile switch clears the whole slate.
        mgr.clear_all_disconnect_intent().await;
        assert!(!mgr.is_disconnect_intended(ChatPlatform::Kick).await);
    }

    /// Changing a channel for a NOT-connected platform is a no-op for the
    /// connection (nothing to drop) and leaves no lingering intent, so the
    /// activation auto-connect is free to connect it on the new channel.
    #[tokio::test]
    async fn channel_change_without_connection_leaves_no_intent() {
        let (mgr, _dir) = manager();
        mgr.update_profile_chat_settings(ChatSettings {
            twitch_channel: "first".into(),
            ..ChatSettings::default()
        })
        .await;
        mgr.update_profile_chat_settings(ChatSettings {
            twitch_channel: "second".into(),
            ..ChatSettings::default()
        })
        .await;
        assert_eq!(mgr.profile_chat_settings().await.twitch_channel, "second");
        assert!(!mgr.is_disconnect_intended(ChatPlatform::Twitch).await);
    }
}
