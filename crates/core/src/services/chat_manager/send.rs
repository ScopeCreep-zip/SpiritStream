use crate::errors::CoreError;
use crate::models::{ChatPlatform, ChatSettings};
use crate::services::{AuditAction, SafetyService};

/// Profile-level send policy: may this platform receive outbound
/// messages at all? Lives in core so an explicit per-message target
/// list from a client can only ever NARROW the broadcast set, never
/// bypass a platform the user disabled.
///
/// Facebook has no per-profile toggle — send is auth-gated by the
/// connector (`can_send()` reflects whether a Page Access Token was
/// captured at connect). TikTok rejects third-party sends by design.
fn send_policy_allows(settings: &ChatSettings, platform: ChatPlatform) -> bool {
    match platform {
        ChatPlatform::Twitch => settings.twitch_send_enabled,
        ChatPlatform::YouTube => settings.youtube_send_enabled && !settings.youtube_use_api_key,
        ChatPlatform::Trovo => settings.trovo_send_enabled,
        ChatPlatform::Kick => settings.kick_send_enabled,
        ChatPlatform::Facebook => true,
        ChatPlatform::TikTok => false,
    }
}

impl super::ChatManager {
    /// Send a chat message to the requested platforms.
    ///
    /// Pipeline:
    /// 1. **PII gate** — if `pii_policy` is `Some((blocklist, fuzzy))`
    ///    and the blocklist is non-empty, run `safety.check_outbound_pii`.
    ///    A match returns `Err(CoreError::ChatBlockedByPii)` for every
    ///    target platform; the message never touches the wire on any
    ///    of them. The audit entry is emitted by `safety` itself.
    /// 2. **Send policy** — the profile's `*_send_enabled` flags, checked
    ///    per platform regardless of how the target list was built.
    /// 3. **Per-platform char-limit** — `ChatPlatform::max_message_chars`.
    /// 4. **Dispatch** — over the live connector.
    /// 5. **Audit** — on at least one successful destination, record
    ///    `ChatMessageSent` with the list of successful platforms and
    ///    the character count. Message text is **never** persisted.
    ///
    /// PII policy snapshots travel by value because callers hold them in
    /// different shapes: HTTP caches `(blocklist, fuzzy)` on
    /// `AppState::active_profile_pii`; CLI loads the active profile
    /// per-command. Both pass through this single function so the
    /// guarantee lives in core, not in any transport.
    pub async fn send_message(
        &self,
        message: String,
        platforms: &[ChatPlatform],
        pii_policy: Option<(Vec<String>, bool)>,
        safety: &SafetyService,
    ) -> Vec<(ChatPlatform, Result<(), CoreError>)> {
        let message_chars = message.chars().count();

        // Step 1: PII gate. Fails all targets atomically — the message
        // either clears every platform or none. The audit + event
        // emission lives inside SafetyService::check_outbound_pii.
        if let Some((blocklist, fuzzy)) = pii_policy {
            if let Err(err) = safety.check_outbound_pii(&blocklist, fuzzy, platforms, &message) {
                return platforms.iter().map(|p| (*p, Err(err.clone()))).collect();
            }
        }

        // Step 1.5: profile send policy. Enforced HERE, not in the
        // transports — an explicit `targetPlatforms` list used to ride
        // straight past the `*_send_enabled` flags.
        let settings = self.profile_chat_settings().await;

        let mut results = Vec::new();
        let mut connectors = self.platforms.lock().await;

        for platform in platforms {
            if !send_policy_allows(&settings, *platform) {
                results.push((
                    *platform,
                    Err(CoreError::ChatSendingDisabled {
                        platform: platform.as_str().to_string(),
                    }),
                ));
                continue;
            }

            // Shared with the crosspost path (`status.rs`) so the two
            // outbound length rules can never drift.
            if let Err(err) = super::check_platform_length(*platform, message_chars) {
                results.push((*platform, Err(err)));
                continue;
            }

            if let Some(connector) = connectors.get_mut(platform) {
                if !connector.can_send() {
                    results.push((
                        *platform,
                        Err(CoreError::ChatSendingDisabled {
                            platform: platform.as_str().to_string(),
                        }),
                    ));
                    continue;
                }

                let result = connector.send_message(message.clone()).await.map_err(|e| {
                    CoreError::Internal {
                        context: format!("Send failed: {}", e),
                    }
                });
                results.push((*platform, result));
            } else {
                results.push((
                    *platform,
                    Err(CoreError::ChatPlatformNotConnected {
                        platform: platform.as_str().to_string(),
                    }),
                ));
            }
        }

        // Step 5: Audit. Record the list of successful destinations.
        // Char count is grapheme-conservative (chars().count()), matching
        // the per-platform limit check above.
        let successes: Vec<String> = results
            .iter()
            .filter(|(_, r)| r.is_ok())
            .map(|(p, _)| p.as_str().to_string())
            .collect();
        if !successes.is_empty() {
            if let Some(audit) = self.audit() {
                let _ = audit.record(AuditAction::ChatMessageSent {
                    platforms: successes,
                    char_count: message_chars,
                });
            }
        }

        results
    }
}
