use crate::errors::CoreError;
use crate::models::ChatPlatform;
use crate::services::{AuditAction, SafetyService};

impl super::ChatManager {
    /// Send a chat message to the requested platforms.
    ///
    /// Pipeline:
    /// 1. **PII gate** — if `pii_policy` is `Some((blocklist, fuzzy))`
    ///    and the blocklist is non-empty, run `safety.check_outbound_pii`.
    ///    A match returns `Err(CoreError::ChatBlockedByPii)` for every
    ///    target platform; the message never touches the wire on any
    ///    of them. The audit entry is emitted by `safety` itself.
    /// 2. **Per-platform char-limit** — `ChatPlatform::max_message_chars`.
    /// 3. **Dispatch** — over the live connector.
    /// 4. **Audit** — on at least one successful destination, record
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

        let mut results = Vec::new();
        let mut connectors = self.platforms.lock().await;

        for platform in platforms {
            let limit = platform.max_message_chars();
            if message_chars > limit {
                results.push((
                    *platform,
                    Err(CoreError::ChatMessageLengthExceeded {
                        platform: platform.as_str().to_string(),
                        limit,
                        actual: message_chars,
                    }),
                ));
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

        // Step 4: Audit. Record the list of successful destinations.
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
