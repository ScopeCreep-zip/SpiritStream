use log::info;

use crate::errors::CoreError;
use crate::models::{ChatConfig, ChatCredentials, ChatPlatform};
use crate::services::AuditAction;

use super::chat_validation;

/// Extract the platform-side identifier from credentials so we can
/// stamp it on the `ChatPlatformConnected` audit entry. Different
/// platforms key on different things:
/// - Twitch / Trovo / Kick: the channel slug the connector subscribed to.
/// - YouTube: the channel id (the watch broadcast id is only known once
///   the live discovery returns; not surfaced at this layer yet).
/// - TikTok: the username.
/// - Facebook: the live video id — load-bearing because it doubles as
///   the audit grep key when reconstructing "did I ever enable Facebook
///   for this stream" history.
fn account_id_from_credentials(creds: &ChatCredentials) -> Option<String> {
    match creds {
        ChatCredentials::Twitch { channel, .. } => Some(channel.clone()),
        ChatCredentials::YouTube { channel_id, .. } => Some(channel_id.clone()),
        ChatCredentials::Trovo { channel_id, .. } => Some(channel_id.clone()),
        ChatCredentials::Kick { channel, .. } => Some(channel.clone()),
        ChatCredentials::TikTok { username, .. } => Some(username.clone()),
        ChatCredentials::Facebook { video_id, .. } => Some(video_id.clone()),
    }
}

impl super::ChatManager {
    /// Connect to a chat platform. Rejects if the platform is already
    /// connected — callers wanting to swap a live session (e.g. read-only →
    /// send-capable after a token refresh) use [`reconnect`].
    pub async fn connect(&self, config: ChatConfig) -> Result<(), CoreError> {
        self.connect_inner(config, false).await
    }

    /// Reconnect a platform, REPLACING any live session. Used to apply a
    /// freshly-refreshed token to a connection that's already up but can't
    /// send (the Twitch anonymous read-only fallback): IRC bakes the token
    /// in at connect, so `update_token` is a no-op there and a clean
    /// disconnect+reconnect under the lock is the only way to swap it.
    pub async fn reconnect(&self, config: ChatConfig) -> Result<(), CoreError> {
        self.connect_inner(config, true).await
    }

    async fn connect_inner(
        &self,
        config: ChatConfig,
        replace_connected: bool,
    ) -> Result<(), CoreError> {
        if !config.enabled {
            return Err(chat_validation(
                "chat_platform_not_enabled",
                "Platform is not enabled",
            ));
        }

        info!("Connecting to {} chat", config.platform.as_str());

        let mut platforms = self.platforms.lock().await;

        // Already connected: reject for a plain connect; for `reconnect`,
        // tear the live session down first so the rebuild starts clean.
        let was_connected = platforms
            .get(&config.platform)
            .map(|c| c.is_connected())
            .unwrap_or(false);
        if was_connected && !replace_connected {
            return Err(chat_validation(
                "chat_platform_already_connected",
                format!("{} is already connected", config.platform.as_str()),
            ));
        }

        // Create or get platform connector. The factory is total over
        // every ChatPlatform variant; `None` is reserved for a future
        // "platform pending re-impl" branch.
        let mut connector = match platforms.remove(&config.platform) {
            Some(existing) => existing,
            None => Self::create_platform_connector(config.platform, &self.chat_endpoints)
                .ok_or_else(|| {
                    chat_validation(
                        "chat_platform_unsupported",
                        format!(
                            "Chat for {} is not yet implemented",
                            config.platform.as_str()
                        ),
                    )
                })?,
        };

        // Forced reconnect over a live session: close the old connection
        // before rebuilding so we don't leave a stale receive loop running.
        if was_connected && replace_connected {
            let _ = connector.disconnect().await;
        }

        // Capture the account_id before the connector consumes
        // `config.credentials`. Powers the audit-log entry below.
        let account_id = account_id_from_credentials(&config.credentials);

        // Capture the Twitch bearer token (if any) before the connector
        // consumes the credentials — the follower-only default below
        // needs it for the Helix chat-settings call.
        let twitch_token: Option<String> = match &config.credentials {
            ChatCredentials::Twitch {
                auth: Some(auth), ..
            } => Some(match auth {
                crate::models::TwitchAuth::UserToken { oauth_token } => oauth_token.clone(),
                crate::models::TwitchAuth::AppOAuth { access_token, .. } => access_token.clone(),
            }),
            _ => None,
        };

        // Connect to the platform. The connector itself tracks its own
        // `last_error` (see `ChatPlatform::last_error` trait impls); re-
        // inserting the failed connector lets the UI surface that error
        // through `get_platform_status` without a separate stale cache.
        let message_tx = self.message_tx.clone();
        if let Err(e) = connector.connect(config.credentials, message_tx).await {
            let error = format!("Failed to connect to {}: {}", config.platform.as_str(), e);
            platforms.insert(config.platform, connector);
            return Err(CoreError::Internal { context: error });
        }

        platforms.insert(config.platform, connector);

        info!("Successfully connected to {}", config.platform.as_str());

        // An explicit successful connect is the user asking for this
        // platform back — clear any deliberate-disconnect intent so the
        // reconnect/auto-connect loops keep it alive from here on.
        self.user_disconnected.lock().await.remove(&config.platform);

        if let Some(audit) = self.audit() {
            let _ = audit.record(AuditAction::ChatPlatformConnected {
                platform: config.platform.as_str().to_string(),
                account_id,
            });
        }

        self.apply_follower_only_default(config.platform, twitch_token)
            .await;

        Ok(())
    }

    /// Apply the profile's follower-only default after a successful
    /// connect. Twitch is the only platform with a server-side API for
    /// it (Helix chat settings); every other platform — and a Twitch
    /// connect without an OAuth token or the required scope — emits a
    /// `follower_only_unsupported` event so the user KNOWS the
    /// protection is not active. Silently ignoring the flag would be a
    /// safety lie to exactly the population this app serves.
    async fn apply_follower_only_default(
        &self,
        platform: ChatPlatform,
        twitch_token: Option<String>,
    ) {
        let enabled = self.chat_settings.lock().await.follower_only_default;
        if !enabled {
            return;
        }
        if platform != ChatPlatform::Twitch {
            self.event_sink.emit(
                "follower_only_unsupported",
                serde_json::json!({
                    "platform": platform.as_str(),
                    "reason": "platform_not_supported",
                }),
            );
            return;
        }
        let Some(token) = twitch_token else {
            log::warn!("follower-only default set but Twitch connected without an OAuth token");
            self.event_sink.emit(
                "follower_only_unsupported",
                serde_json::json!({ "platform": "twitch", "reason": "no_oauth_token" }),
            );
            return;
        };
        let endpoints = self.chat_endpoints.clone();
        let events = self.event_sink.clone();
        // Two HTTP round-trips — run off the connect path; the outcome
        // events keep it observable either way.
        tokio::spawn(async move {
            match crate::services::chat::twitch::room_settings::apply_follower_only_default(
                &endpoints, &token,
            )
            .await
            {
                Ok(()) => {
                    log::info!("follower-only default applied to Twitch chat");
                    events.emit(
                        "follower_only_applied",
                        serde_json::json!({ "platform": "twitch" }),
                    );
                }
                Err(e) => {
                    log::warn!("follower-only default could not be applied: {e}");
                    // Surface the specific validation code (e.g.
                    // `follower_only_missing_scope`) instead of the flat
                    // `validation_failed` kind — the UI prompts a Twitch
                    // re-auth only for the missing-scope case.
                    let reason = match &e {
                        CoreError::ValidationFailed { reasons } if !reasons.is_empty() => {
                            reasons[0].code.clone()
                        }
                        other => other.kind().to_string(),
                    };
                    events.emit(
                        "follower_only_unsupported",
                        serde_json::json!({ "platform": "twitch", "reason": reason }),
                    );
                }
            }
        });
    }

    /// Disconnect from a chat platform.
    pub async fn disconnect(&self, platform: ChatPlatform) -> Result<(), CoreError> {
        info!("Disconnecting from {} chat", platform.as_str());

        // Record the deliberate-disconnect intent BEFORE the await so a
        // concurrent reconnect can't slip in and re-establish the
        // platform between disconnect and the flag being set.
        self.user_disconnected.lock().await.insert(platform);
        // Drop the stale activity timestamp — a disconnected platform has
        // no "last message Xs ago" to advertise.
        self.last_activity.lock().await.remove(&platform);

        let mut platforms = self.platforms.lock().await;

        if let Some(mut connector) = platforms.remove(&platform) {
            connector
                .disconnect()
                .await
                .map_err(|e| CoreError::Internal {
                    context: format!("Failed to disconnect from {}: {}", platform.as_str(), e),
                })?;

            // Re-insert the disconnected connector.
            platforms.insert(platform, connector);

            info!("Successfully disconnected from {}", platform.as_str());

            if let Some(audit) = self.audit() {
                let _ = audit.record(AuditAction::ChatPlatformDisconnected {
                    platform: platform.as_str().to_string(),
                    reason: "user_requested".to_string(),
                });
            }

            Ok(())
        } else {
            Err(CoreError::ChatPlatformNotConnected {
                platform: platform.as_str().to_string(),
            })
        }
    }

    /// Update the OAuth access token for a specific platform (if connected).
    pub async fn update_platform_token(
        &self,
        platform: ChatPlatform,
        token: String,
    ) -> Result<(), CoreError> {
        let mut platforms = self.platforms.lock().await;

        if let Some(connector) = platforms.get_mut(&platform) {
            connector.update_token(token);
            Ok(())
        } else {
            Err(CoreError::ChatPlatformNotConnected {
                platform: platform.as_str().to_string(),
            })
        }
    }

    /// Disconnect from every connected platform. `reason` is recorded
    /// against each per-platform `ChatPlatformDisconnected` audit entry
    /// so a forensic reader can distinguish a panic teardown from a
    /// user-requested mass-disconnect from a process shutdown. Use the
    /// stable short discriminators documented on the audit variant:
    /// `"panic_triggered"`, `"user_requested"`, `"shutdown"`.
    ///
    /// Lock discipline matters here because the panic button calls this:
    /// connectors are taken OUT of the map first, the `platforms` lock is
    /// released, then every disconnect runs concurrently under a 5s
    /// per-connector timeout. The previous implementation held the lock
    /// across sequential disconnect awaits — one hung connector stalled
    /// the entire panic teardown plus every concurrent status/send call.
    pub async fn disconnect_all(&self, reason: &str) -> Result<(), CoreError> {
        info!("Disconnecting from all chat platforms (reason={reason})");

        // Panic = "make it disappear". Stamp the purge boundary FIRST, before
        // the (up-to-5s-per-connector) socket teardown — otherwise messages
        // already buffered in the handler channel would drain and EMIT to the
        // frontend during teardown, repopulating the view AFTER the panic-
        // hotkey cleared it. With the boundary set up front, the message
        // handler's gate drops every straggler (timestamp <= boundary) for
        // the whole teardown window: it can't reach disk, the ring, or the
        // UI. The ring/history/activity wipe still runs at the end to clear
        // anything that landed before this instant.
        if reason == "panic_triggered" {
            self.purge_epoch_ms.store(
                chrono::Utc::now().timestamp_millis(),
                std::sync::atomic::Ordering::SeqCst,
            );
        }

        const DISCONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

        let mut to_disconnect: Vec<(ChatPlatform, crate::services::chat::BoxedPlatform)> = {
            let mut platforms = self.platforms.lock().await;
            let connected: Vec<ChatPlatform> = platforms
                .iter()
                .filter(|(_, c)| c.is_connected())
                .map(|(p, _)| *p)
                .collect();
            connected
                .into_iter()
                .filter_map(|p| platforms.remove(&p).map(|c| (p, c)))
                .collect()
        };

        let outcomes = futures_util::future::join_all(to_disconnect.iter_mut().map(
            |(platform, connector)| {
                let platform = *platform;
                async move {
                    match tokio::time::timeout(DISCONNECT_TIMEOUT, connector.disconnect()).await {
                        Ok(Ok(())) => (platform, Ok(())),
                        Ok(Err(e)) => (platform, Err(e.to_string())),
                        Err(_) => (
                            platform,
                            Err(format!(
                                "disconnect timed out after {}s",
                                DISCONNECT_TIMEOUT.as_secs()
                            )),
                        ),
                    }
                }
            },
        ))
        .await;

        // Put the (now-disconnected) connectors back so platform state
        // queries stay consistent with the pre-call shape.
        {
            let mut platforms = self.platforms.lock().await;
            for (platform, connector) in to_disconnect {
                platforms.insert(platform, connector);
            }
        }

        let mut errors = Vec::new();
        let mut disconnected: Vec<ChatPlatform> = Vec::new();
        for (platform, outcome) in outcomes {
            match outcome {
                Ok(()) => disconnected.push(platform),
                Err(e) => errors.push(format!("{}: {}", platform.as_str(), e)),
            }
        }

        // A panic or an explicit "disconnect all" must STAY down — record
        // intent for every platform we just tore down so the reconnect /
        // auto-connect loops don't silently bring chat back. A `shutdown`
        // teardown is not a user "go silent" signal, so it leaves intent
        // untouched (chat returns on next startup as expected).
        if reason == "panic_triggered" || reason == "user_requested" {
            let mut intent = self.user_disconnected.lock().await;
            for platform in &disconnected {
                intent.insert(*platform);
            }
        }
        // Disconnected platforms have no current activity to advertise.
        {
            let mut activity = self.last_activity.lock().await;
            for platform in &disconnected {
                activity.remove(platform);
            }
        }

        // The purge boundary was already stamped at the top of this fn (so
        // the handler gate covered the whole teardown). Now wipe what's
        // already stored: the in-memory ring, the activity timestamps, and
        // the on-disk history.
        if reason == "panic_triggered" {
            self.clear_recent_messages().await;
            self.last_activity.lock().await.clear();
            self.purge_chat_history();
        }

        if let Some(audit) = self.audit() {
            for platform in &disconnected {
                let _ = audit.record(AuditAction::ChatPlatformDisconnected {
                    platform: platform.as_str().to_string(),
                    reason: reason.to_string(),
                });
            }
        }

        if errors.is_empty() {
            info!("Successfully disconnected from all platforms");
            Ok(())
        } else {
            Err(CoreError::Internal {
                context: format!("Some disconnections failed: {}", errors.join(", ")),
            })
        }
    }
}
