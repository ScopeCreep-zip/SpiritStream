use crate::errors::CoreError;
use crate::services::{media_sanitizer, validate_path_within_any};
use log::{error, info, warn};
use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

#[derive(Debug, Serialize)]
struct WebhookPayload {
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RateLimitResponse {
    retry_after: f64,
    /// Discord returns this on 429; operators care because `true`
    /// means *every* webhook send is blocked for `retry_after`, not
    /// just this one URL — useful signal in support tickets.
    global: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookResult {
    pub success: bool,
    pub message: String,
    pub skipped_cooldown: bool,
}

pub struct DiscordWebhookService {
    client: reqwest::Client,
    /// Monotonic instant of the last send-attempt that passed the cooldown
    /// gate. Held under a Mutex (not RwLock) so check-and-set is atomic —
    /// two concurrent sends can't both pass the read-side check and then
    /// both update. Using `Instant` (monotonic) instead of `Utc::now()`
    /// means a wall-clock jump backward (NTP correction) doesn't make the
    /// cooldown elapse instantly. The cooldown reserves the slot before
    /// the HTTP send so a failed-then-retried send still respects the
    /// window — retries shouldn't bypass it.
    last_send_instant: Arc<Mutex<Option<Instant>>>,
    /// Allowed roots a user-supplied `image_path` must canonicalise
    /// inside before we'll read it. Populated by `ServiceRegistry`
    /// from the app data dir + the user's standard Pictures/Downloads
    /// directories. An attacker who controls the profile JSON can put
    /// `/etc/passwd` (or `C:\Windows\…`) in `discord.image_path`;
    /// without this gate the bytes would leave the host as a Discord
    /// attachment.
    allowed_image_roots: Arc<Vec<PathBuf>>,
    /// Audit-log handle, wired post-construction by `ServiceRegistry`
    /// (same pattern as `ProfileManager::set_audit_log`). H9: webhook
    /// sends are user-visible side effects and must reach the HMAC
    /// chain. Before wiring, sends still succeed — they just don't
    /// record.
    audit_log: Arc<std::sync::RwLock<Option<Arc<crate::services::AuditLogService>>>>,
}

impl DiscordWebhookService {
    /// Create a new Discord webhook service. `data_dir` anchors the
    /// allowed-roots list so user-supplied `image_path` values cannot
    /// escape sensible bounds. Standard user dirs (Pictures/Downloads/
    /// Documents/Desktop/Home) are added when discoverable so a real
    /// streamer can point at their own photo without the validator
    /// rejecting it.
    pub fn new(data_dir: PathBuf) -> Self {
        let mut roots: Vec<PathBuf> = vec![data_dir];
        // Order matters only for error messages; the validator returns
        // success on the first matching root.
        for candidate in [
            dirs_next::picture_dir(),
            dirs_next::download_dir(),
            dirs_next::document_dir(),
            dirs_next::desktop_dir(),
            dirs_next::home_dir(),
        ]
        .into_iter()
        .flatten()
        {
            if !roots.iter().any(|r| r == &candidate) {
                roots.push(candidate);
            }
        }
        Self {
            client: reqwest::Client::new(),
            last_send_instant: Arc::new(Mutex::new(None)),
            allowed_image_roots: Arc::new(roots),
            audit_log: Arc::new(std::sync::RwLock::new(None)),
        }
    }

    /// Wire the audit log post-construction. Mirrors `ChatManager::set_audit_log`.
    pub fn set_audit_log(&self, audit: Arc<crate::services::AuditLogService>) {
        match self.audit_log.write() {
            Ok(mut guard) => *guard = Some(audit),
            Err(e) => {
                error!("discord_webhook audit_log write lock poisoned during set_audit_log: {e}")
            }
        }
    }

    /// Send a go-live notification
    ///
    /// # Arguments
    /// * `webhook_url` - Discord webhook URL
    /// * `message` - Message content (supports Discord markdown)
    /// * `image_path` - Optional path to an image file to attach
    /// * `cooldown_enabled` - Whether cooldown is enabled
    /// * `cooldown_seconds` - Cooldown period in seconds
    pub async fn send_go_live_notification(
        &self,
        webhook_url: &str,
        message: &str,
        image_path: Option<&str>,
        cooldown_enabled: bool,
        cooldown_seconds: u32,
    ) -> WebhookResult {
        if webhook_url.is_empty() {
            return WebhookResult {
                success: false,
                message: "Webhook URL is not configured".to_string(),
                skipped_cooldown: false,
            };
        }

        if !webhook_url.starts_with("https://discord.com/api/webhooks/")
            && !webhook_url.starts_with("https://discordapp.com/api/webhooks/")
        {
            return WebhookResult {
                success: false,
                message: "Invalid Discord webhook URL".to_string(),
                skipped_cooldown: false,
            };
        }

        // Check cooldown + reserve the slot atomically. Hold the mutex
        // across both the elapsed check AND the timestamp update so two
        // concurrent callers can't both pass the gate. Slot is reserved
        // BEFORE the HTTP send so a failed-then-retried send still
        // respects the cooldown — retries shouldn't bypass it.
        if cooldown_enabled {
            let cooldown_duration = std::time::Duration::from_secs(cooldown_seconds as u64);
            let mut last_send = self.last_send_instant.lock().await;
            if let Some(last_time) = *last_send {
                let elapsed = last_time.elapsed();
                if elapsed < cooldown_duration {
                    let remaining = (cooldown_duration - elapsed).as_secs();
                    info!(
                        "Discord notification skipped: cooldown active ({} seconds remaining)",
                        remaining
                    );
                    let outcome = WebhookResult {
                        success: true,
                        message: format!(
                            "Notification skipped: cooldown active ({} seconds remaining)",
                            remaining
                        ),
                        skipped_cooldown: true,
                    };
                    drop(last_send);
                    self.record_send_audit(&outcome);
                    return outcome;
                }
            }
            // Reserve the cooldown slot now — see field comment for why.
            *last_send = Some(Instant::now());
        }

        let payload = WebhookPayload {
            content: message.to_string(),
            username: Some("SpiritStream".to_string()),
            avatar_url: None,
        };

        let result = if let Some(path) = image_path {
            if !path.is_empty() && Path::new(path).exists() {
                self.send_webhook_with_image(webhook_url, &payload, path)
                    .await
            } else {
                if !path.is_empty() {
                    warn!("Discord image file not found: {}", path);
                }
                self.send_webhook(webhook_url, &payload).await
            }
        } else {
            self.send_webhook(webhook_url, &payload).await
        };

        let outcome = match result {
            Ok(()) => {
                // Cooldown slot was already reserved in the check above.
                // No need to update last_send_instant a second time.
                info!("Discord go-live notification sent successfully");
                WebhookResult {
                    success: true,
                    message: "Notification sent successfully".to_string(),
                    skipped_cooldown: false,
                }
            }
            Err(e) => {
                error!("Failed to send Discord notification: {}", e);
                WebhookResult {
                    success: false,
                    message: format!("Failed to send notification: {}", e),
                    skipped_cooldown: false,
                }
            }
        };
        self.record_send_audit(&outcome);
        outcome
    }

    fn record_send_audit(&self, result: &WebhookResult) {
        // Audit gaps are compliance-critical; surface lock poisoning and
        // failed appends loudly so operators see the silence instead of
        // discovering it during an incident. See feedback_no_fallback_streaming.md.
        let audit = match self.audit_log.read() {
            Ok(guard) => guard.clone(),
            Err(e) => {
                error!(
                    "discord_webhook audit_log read lock poisoned — DiscordWebhookSent not recorded: {e}"
                );
                return;
            }
        };
        let Some(audit) = audit else { return };
        if let Err(e) = audit.record(crate::services::AuditAction::DiscordWebhookSent {
            success: result.success,
            skipped_cooldown: result.skipped_cooldown,
        }) {
            error!("discord_webhook failed to append DiscordWebhookSent audit entry: {e}");
        }
    }

    /// Send a webhook request to Discord
    async fn send_webhook(&self, url: &str, payload: &WebhookPayload) -> Result<(), CoreError> {
        let response = self
            .client
            .post(url)
            .json(payload)
            .send()
            .await
            .map_err(|e| CoreError::NetworkError {
                detail: format!("Request failed: {e}"),
            })?;

        self.handle_response(response).await
    }

    /// Send a webhook request with an image attachment. The path is
    /// validated against `allowed_image_roots` (F4) and the bytes are
    /// stripped of identifying metadata via `media_sanitizer` (F3)
    /// before they leave the host. EXIF GPS, ICC profile fingerprints,
    /// and PNG text chunks would otherwise dox a streamer who attached
    /// a photo taken on their phone.
    async fn send_webhook_with_image(
        &self,
        url: &str,
        payload: &WebhookPayload,
        image_path: &str,
    ) -> Result<(), CoreError> {
        // F4 — refuse anything outside the allowlist before the host
        // filesystem is even touched. The validator returns a
        // canonicalised path on success.
        let allowed_refs: Vec<&Path> = self
            .allowed_image_roots
            .iter()
            .map(|p| p.as_path())
            .collect();
        let validated_path = validate_path_within_any(Path::new(image_path), &allowed_refs)?;

        let raw_bytes =
            tokio::fs::read(&validated_path)
                .await
                .map_err(|e| CoreError::NetworkError {
                    detail: format!("Failed to read image file: {e}"),
                })?;

        // F3 — strip metadata. `strip_metadata` refuses unrecognised
        // formats; that refusal becomes a clear validation error
        // rather than shipping un-scrubbed bytes through Discord.
        let image_data = media_sanitizer::strip_metadata(&raw_bytes)?;

        let file_name = validated_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("image.png")
            .to_string();

        let mime_type = match validated_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .as_deref()
        {
            Some("png") => "image/png",
            Some("jpg") | Some("jpeg") => "image/jpeg",
            Some("gif") => "image/gif",
            Some("webp") => "image/webp",
            _ => "application/octet-stream",
        };

        let payload_json = serde_json::to_string(payload).map_err(|e| CoreError::Internal {
            context: format!("Failed to serialize Discord payload: {e}"),
        })?;

        let file_part = Part::bytes(image_data)
            .file_name(file_name)
            .mime_str(mime_type)
            .map_err(|e| CoreError::NetworkError {
                detail: format!("Failed to create file part: {e}"),
            })?;

        let form = Form::new()
            .text("payload_json", payload_json)
            .part("file", file_part);

        let response = self
            .client
            .post(url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| CoreError::NetworkError {
                detail: format!("Request failed: {e}"),
            })?;

        self.handle_response(response).await
    }

    /// Handle Discord API response
    async fn handle_response(&self, response: reqwest::Response) -> Result<(), CoreError> {
        let status = response.status();

        if status.is_success() || status.as_u16() == 204 {
            // 204 No Content is the normal success response for webhooks
            Ok(())
        } else if status.as_u16() == 429 {
            let rate_limit: RateLimitResponse =
                response.json().await.map_err(|e| CoreError::NetworkError {
                    detail: format!("Failed to parse rate limit response: {e}"),
                })?;

            let scope = if rate_limit.global {
                "global"
            } else {
                "per-route"
            };
            warn!(
                "Discord rate limit hit ({scope}), retry after {} seconds",
                rate_limit.retry_after
            );
            Err(CoreError::NetworkError {
                detail: format!(
                    "Rate limited by Discord ({scope}). Try again in {:.1} seconds",
                    rate_limit.retry_after
                ),
            })
        } else if status.as_u16() == 400 {
            // Bad request - usually means invalid message content
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(CoreError::NetworkError {
                detail: format!("Invalid request: {error_text}"),
            })
        } else if status.as_u16() == 401 || status.as_u16() == 403 {
            Err(CoreError::NetworkError {
                detail: "Invalid webhook URL or webhook has been deleted".to_string(),
            })
        } else if status.as_u16() == 404 {
            Err(CoreError::NetworkError {
                detail: "Webhook not found - the webhook may have been deleted".to_string(),
            })
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            Err(CoreError::NetworkError {
                detail: format!("Discord API error ({status}): {error_text}"),
            })
        }
    }

    /// Test a webhook URL by sending a test message
    pub async fn test_webhook(&self, webhook_url: &str) -> WebhookResult {
        if webhook_url.is_empty() {
            return WebhookResult {
                success: false,
                message: "Webhook URL is empty".to_string(),
                skipped_cooldown: false,
            };
        }

        if !webhook_url.starts_with("https://discord.com/api/webhooks/")
            && !webhook_url.starts_with("https://discordapp.com/api/webhooks/")
        {
            return WebhookResult {
                success: false,
                message: "Invalid Discord webhook URL format".to_string(),
                skipped_cooldown: false,
            };
        }

        let payload = WebhookPayload {
            content: "🔔 **SpiritStream Test** - Webhook connection successful!".to_string(),
            username: Some("SpiritStream".to_string()),
            avatar_url: None,
        };

        match self.send_webhook(webhook_url, &payload).await {
            Ok(()) => {
                info!("Discord webhook test successful");
                WebhookResult {
                    success: true,
                    message: "Test message sent successfully".to_string(),
                    skipped_cooldown: false,
                }
            }
            Err(e) => {
                warn!("Discord webhook test failed: {}", e);
                WebhookResult {
                    success: false,
                    message: e.to_string(),
                    skipped_cooldown: false,
                }
            }
        }
    }

    /// Reset the cooldown timer (useful for testing)
    pub async fn reset_cooldown(&self) {
        let mut last_send = self.last_send_instant.lock().await;
        *last_send = None;
        info!("Discord webhook cooldown reset");
    }
}

impl Default for DiscordWebhookService {
    fn default() -> Self {
        // Default cannot pull a sensible data_dir; fall back to the
        // current directory so callers that lean on `Default::default`
        // (tests, examples) still construct. Production wiring goes
        // through `ServiceRegistry::build` which passes the real path.
        Self::new(PathBuf::from("."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_webhook_url_validation() {
        let service = DiscordWebhookService::new(PathBuf::from("."));

        // We can't actually test the webhook without a real URL,
        // but we can verify the service initializes correctly.
        // `try_lock` succeeds when no other task holds the lock — at
        // construction time nothing else has touched it yet.
        assert!(service.last_send_instant.try_lock().is_ok());
    }

    /// F4 regression: a traversal path must not be readable.
    #[tokio::test]
    async fn image_path_traversal_is_rejected() {
        let dir = TempDir::new().unwrap();
        let svc = DiscordWebhookService::new(dir.path().to_path_buf());
        let payload = WebhookPayload {
            content: "test".into(),
            username: None,
            avatar_url: None,
        };
        // Resolve to an absolute path with `..` segments so we exercise
        // the traversal-detection branch of validate_path_within rather
        // than depending on a particular target file existing.
        let bad = dir.path().join("../../../etc/passwd");
        let err = svc
            .send_webhook_with_image(
                "https://discord.com/api/webhooks/0/x",
                &payload,
                bad.to_str().unwrap(),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, CoreError::PathOutsideAllowedRoot { .. }),
            "expected PathOutsideAllowedRoot, got {err:?}",
        );
    }

    /// Q9: cooldown gate. With `cooldown_enabled=true` and a high
    /// `cooldown_seconds` (so the test doesn't have to actually wait
    /// out the window), the second send must return
    /// `skipped_cooldown=true` and short-circuit before the network.
    #[tokio::test]
    async fn cooldown_skips_second_send_within_window() {
        let dir = TempDir::new().unwrap();
        let svc = DiscordWebhookService::new(dir.path().to_path_buf());

        // Fake a recent successful send by seeding `last_send_instant`.
        {
            let mut guard = svc.last_send_instant.lock().await;
            *guard = Some(Instant::now());
        }

        let result = svc
            .send_go_live_notification(
                "https://discord.com/api/webhooks/0/x",
                "test",
                None,
                /* cooldown_enabled */ true,
                /* cooldown_seconds */ 3600,
            )
            .await;
        assert!(result.success, "cooldown skip is reported as success");
        assert!(
            result.skipped_cooldown,
            "expected skipped_cooldown=true; got {result:?}",
        );
        assert!(result.message.to_lowercase().contains("cooldown"));
    }

    /// Q9: cooldown window elapsed → send proceeds. The downstream
    /// HTTP attempt fails (no real webhook server) so we only assert
    /// `skipped_cooldown=false`, meaning the gate admitted the call.
    #[tokio::test]
    async fn cooldown_window_elapsed_lets_send_proceed() {
        let dir = TempDir::new().unwrap();
        let svc = DiscordWebhookService::new(dir.path().to_path_buf());
        // Seed an instant from 2 hours ago — monotonic clock can't go
        // negative, so use `checked_sub` and skip the test if the
        // process hasn't been alive long enough (won't be in practice).
        let past = Instant::now().checked_sub(std::time::Duration::from_secs(7200));
        if let Some(past_instant) = past {
            let mut guard = svc.last_send_instant.lock().await;
            *guard = Some(past_instant);
        } else {
            // Process just started; clear the slot so the send proceeds.
            let mut guard = svc.last_send_instant.lock().await;
            *guard = None;
        }
        let result = svc
            .send_go_live_notification(
                "https://discord.com/api/webhooks/0/x",
                "test",
                None,
                true,
                3600,
            )
            .await;
        assert!(
            !result.skipped_cooldown,
            "expected fresh window: {result:?}",
        );
    }

    /// O.8a regression: two concurrent sends must NOT both pass the
    /// cooldown gate. Pre-fix the gate was check-then-act under
    /// separate RwLock acquisitions with the HTTP request in between
    /// — both callers could observe `last_send=None`, both skip the
    /// check, both attempt the send, both update timestamps. The
    /// monotonic Mutex check-and-set ensures one wins outright.
    #[tokio::test]
    async fn concurrent_sends_serialize_through_cooldown_gate() {
        let dir = TempDir::new().unwrap();
        let svc = Arc::new(DiscordWebhookService::new(dir.path().to_path_buf()));

        // Two concurrent first-sends. Both see `last_send_instant=None`
        // initially; one must reserve the slot, the other must be told
        // skipped_cooldown=true. Without the atomic check-and-set both
        // would attempt the network call.
        let svc_a = svc.clone();
        let svc_b = svc.clone();
        let url = "https://discord.com/api/webhooks/0/x";
        let (result_a, result_b) = tokio::join!(
            svc_a.send_go_live_notification(url, "msg-a", None, true, 3600),
            svc_b.send_go_live_notification(url, "msg-b", None, true, 3600),
        );
        // Exactly one of the two must report `skipped_cooldown=true`.
        // The other proceeded past the gate (and likely got a network
        // error since there's no real webhook server, but that's fine
        // — we only care that the gate didn't admit both).
        let skip_count = [&result_a, &result_b]
            .iter()
            .filter(|r| r.skipped_cooldown)
            .count();
        assert_eq!(
            skip_count, 1,
            "exactly one concurrent send should be cooldown-skipped; got results: a={result_a:?} b={result_b:?}",
        );
    }

    /// Q9: `reset_cooldown` clears the last-send timestamp so the
    /// next call always proceeds regardless of recent history.
    #[tokio::test]
    async fn reset_cooldown_unblocks_immediate_resend() {
        let dir = TempDir::new().unwrap();
        let svc = DiscordWebhookService::new(dir.path().to_path_buf());
        {
            let mut guard = svc.last_send_instant.lock().await;
            *guard = Some(Instant::now());
        }
        svc.reset_cooldown().await;
        let after = svc.last_send_instant.lock().await;
        assert!(
            after.is_none(),
            "reset_cooldown must clear last_send_instant"
        );
    }

    /// F3 regression: an in-allowlist JPEG with an EXIF segment must
    /// be stripped before reaching the multipart writer. We can't
    /// observe the request bytes without a mock server, so this test
    /// invokes `media_sanitizer::strip_metadata` on the same input the
    /// service would and asserts the EXIF marker is gone.
    #[tokio::test]
    async fn image_metadata_is_stripped_before_upload() {
        let dir = TempDir::new().unwrap();
        let img_path = dir.path().join("photo.jpg");
        // Minimal JPEG with an APP1/EXIF marker (FF E1) and the "Exif\0\0"
        // header. Smallest payload accepted by img-parts: SOI + APP1 +
        // EOI. Pixel data is irrelevant for the metadata-strip check.
        let mut bytes: Vec<u8> = Vec::new();
        bytes.extend_from_slice(&[0xFF, 0xD8]); // SOI
        bytes.extend_from_slice(&[0xFF, 0xE1, 0x00, 0x0E]); // APP1, len=14
        bytes.extend_from_slice(b"Exif\0\0"); // EXIF header
        bytes.extend_from_slice(b"XXXXXX"); // 6 bytes filler so len matches
        bytes.extend_from_slice(&[0xFF, 0xD9]); // EOI
        std::fs::File::create(&img_path)
            .unwrap()
            .write_all(&bytes)
            .unwrap();

        let stripped = media_sanitizer::strip_metadata(&std::fs::read(&img_path).unwrap()).unwrap();
        // After strip, the EXIF segment is gone — no "Exif\0\0" header.
        // (img-parts may re-emit a minimal JPEG without an EOI for this
        // hand-built fixture; the load-bearing assertion for F3 is that
        // the EXIF segment is GONE, not the exact byte layout.)
        assert!(
            !stripped.windows(6).any(|w| w == b"Exif\0\0"),
            "EXIF header survived strip_metadata",
        );
        // SOI marker preserved (still recognisable as a JPEG).
        assert_eq!(&stripped[..2], &[0xFF, 0xD8]);

        // Sanity: the service builds without panic for the in-allowlist
        // path. We don't actually POST to Discord — that requires a
        // mock server, which is N1's territory.
        let _svc = DiscordWebhookService::new(dir.path().to_path_buf());
    }
}
