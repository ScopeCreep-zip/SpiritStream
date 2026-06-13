//! `spiritstream-cli chat …` — chat platform connections.

use clap::Subcommand;
use spiritstream_core::models::{
    ChatConfig, ChatCredentials, ChatPlatform, ChatSendResult, TwitchAuth, YouTubeAuth,
};
use spiritstream_core::ServiceRegistry;

use crate::error::CliError;
use crate::output::Output;

fn parse_platform(raw: &str) -> Result<ChatPlatform, CliError> {
    // The model derives `Deserialize` with `rename_all = "lowercase"` (plus a
    // dedicated `tiktok` rename) — round-trip through serde so the CLI accepts
    // exactly the same wire values the HTTP surface does.
    serde_json::from_value(serde_json::json!(raw))
        .map_err(|e| CliError::Argument(format!("unknown chat platform '{raw}': {e}")))
}

/// Translate per-platform CLI flags into a `ChatConfig`. The HTTP equivalent
/// (`v1_chat_connect_proxy`) also performs OAuth-refresh enrichment from the
/// active profile; the CLI path keeps credentials explicit — the user
/// supplies `--oauth` themselves or runs `oauth refresh` separately. This
/// keeps each transport's path narrow and uniformly typed.
fn build_connect_config(
    platform: ChatPlatform,
    channel: Option<String>,
    channel_id: Option<String>,
    oauth: Option<String>,
    api_key: Option<String>,
    session_token: Option<String>,
    use_api_key: bool,
    trovo_client_id: String,
) -> Result<ChatConfig, CliError> {
    let credentials = match platform {
        ChatPlatform::Twitch => {
            let channel = channel
                .ok_or_else(|| CliError::Argument("twitch connect requires --channel".into()))?;
            let auth = oauth.map(|access_token| TwitchAuth::AppOAuth {
                access_token,
                refresh_token: None,
                expires_at: None,
            });
            ChatCredentials::Twitch { channel, auth }
        }
        ChatPlatform::YouTube => {
            let channel_id = channel_id.ok_or_else(|| {
                CliError::Argument("youtube connect requires --channel-id".into())
            })?;
            let auth = if use_api_key {
                let key = api_key
                    .ok_or_else(|| CliError::Argument("--use-api-key requires --api-key".into()))?;
                YouTubeAuth::ApiKey { key }
            } else {
                let access_token = oauth.ok_or_else(|| {
                    CliError::Argument(
                        "youtube connect requires --oauth (or pass --use-api-key with --api-key)"
                            .into(),
                    )
                })?;
                YouTubeAuth::AppOAuth {
                    access_token,
                    refresh_token: None,
                    expires_at: None,
                }
            };
            ChatCredentials::YouTube { channel_id, auth }
        }
        ChatPlatform::Trovo => {
            let channel_id = channel_id
                .or(channel)
                .ok_or_else(|| CliError::Argument("trovo connect requires --channel-id".into()))?;
            // Optional send credential via the same `--oauth-from`
            // source the other platforms use; read-only without it.
            ChatCredentials::Trovo {
                channel_id,
                // Resolved from the OAuth config chain (in-app setup →
                // env → embedded) by the caller; the connector fails
                // loud on a placeholder.
                client_id: Some(trovo_client_id),
                oauth_token: oauth,
            }
        }
        ChatPlatform::TikTok => {
            let username = channel.ok_or_else(|| {
                CliError::Argument("tiktok connect requires --channel (username)".into())
            })?;
            ChatCredentials::TikTok {
                username,
                session_token,
            }
        }
        ChatPlatform::Kick | ChatPlatform::Facebook => {
            return Err(CliError::Argument(format!(
                "{} chat is not yet implemented",
                platform.as_str()
            )));
        }
    };
    Ok(ChatConfig {
        platform,
        enabled: true,
        credentials,
    })
}

#[derive(Debug, Subcommand)]
pub enum ChatCmd {
    /// Per-platform chat connection status.
    Status,
    /// Send a chat message to every enabled platform (the same set
    /// `ChatService::profile_chat_settings` exposes — `*_send_enabled`
    /// flags decide where it goes). The per-platform char limit lives
    /// in core (`ChatPlatform::max_message_chars`) and applies here too.
    Send { message: String },
    /// Connect to a chat platform. Credentials are sourced from explicit
    /// flags rather than the active profile so the CLI works without
    /// loading a profile first; for OAuth-bearing platforms, supply the
    /// access token via `--oauth`.
    Connect {
        /// Platform (twitch / youtube / trovo / tiktok).
        platform: String,
        /// Twitch channel name, Trovo channel ID, etc.
        #[arg(long)]
        channel: Option<String>,
        /// YouTube channel ID (`UC…` or `@handle`).
        #[arg(long)]
        channel_id: Option<String>,
        /// Source for the OAuth access token (Twitch / YouTube AppOAuth).
        /// Secrets never ride argv — pipe via stdin or use the prompt.
        #[arg(long = "oauth-from", value_enum)]
        oauth_from: Option<crate::secret_input::SecretSource>,
        /// Source for the YouTube API key (for `--use-api-key` mode).
        #[arg(long = "api-key-from", value_enum)]
        api_key_from: Option<crate::secret_input::SecretSource>,
        /// Source for the TikTok session cookie / token.
        #[arg(long = "session-token-from", value_enum)]
        session_token_from: Option<crate::secret_input::SecretSource>,
        /// Use the YouTube API-key auth path instead of OAuth.
        #[arg(long)]
        use_api_key: bool,
    },
    /// Reconnect a chat platform. Same credential shape as `connect` —
    /// `retry` first disconnects (silently if not connected) and then
    /// connects. Matches the HTTP `POST /chat/connections/:platform/retry`
    /// endpoint's effective behaviour.
    Retry {
        platform: String,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long)]
        channel_id: Option<String>,
        #[arg(long = "oauth-from", value_enum)]
        oauth_from: Option<crate::secret_input::SecretSource>,
        #[arg(long = "api-key-from", value_enum)]
        api_key_from: Option<crate::secret_input::SecretSource>,
        #[arg(long = "session-token-from", value_enum)]
        session_token_from: Option<crate::secret_input::SecretSource>,
        #[arg(long)]
        use_api_key: bool,
    },
    /// Disconnect a specific platform (twitch / youtube / trovo / kick /
    /// facebook / tiktok).
    Disconnect { platform: String },
    /// Disconnect every connected chat platform.
    DisconnectAll,
    /// Print whether any chat platform is connected (boolean).
    Connected,
    /// Print the current chat log session start timestamp (or `null`).
    LogStatus,
    /// Print the status for a single platform. (`PlatformStatus`
    /// avoids the clap-derived `status1` slug the auto-numbered name
    /// produced — the original `Status1` came from the second `Status`
    /// variant being silently renumbered, surfaced as a confusing CLI
    /// command name to operators.)
    PlatformStatus { platform: String },
    /// Export the current chat log session to a file.
    /// Export the active session's chat log to `<path>`. The
    /// destination must lie inside the data directory or the user's
    /// home (same policy as `system logs-export`).
    ExportLog { path: std::path::PathBuf },
    /// Search the current chat log session.
    Search {
        query: String,
        #[arg(long)]
        limit: Option<usize>,
    },
}

pub async fn run(
    cmd: ChatCmd,
    registry: &ServiceRegistry,
    out: &mut Output,
) -> Result<(), CliError> {
    match cmd {
        ChatCmd::Status => {
            let status = registry.chat.get_status().await;
            out.emit(&status)?;
            Ok(())
        }
        ChatCmd::Send { message } => {
            let trimmed = message.trim().to_string();
            if trimmed.is_empty() {
                return Err(CliError::Argument("message cannot be empty".into()));
            }

            // CLI is stateless per-invocation — registry.chat's cached
            // chat_settings + pii cache are empty on cold start. Load
            // the active profile from disk and derive both targets and
            // PII policy from one read.
            let global = registry.settings.load().ok();
            let active_name = global.as_ref().and_then(|s| s.last_profile.clone());
            let active_profile = match active_name.as_ref() {
                Some(name) => registry.profiles.load(name, None).await.ok(),
                None => None,
            };

            let chat_cfg = active_profile
                .as_ref()
                .map(|p| p.settings.chat.clone())
                .unwrap_or_default();
            // Rehydrate the manager's settings cache: core's send path
            // enforces the `*_send_enabled` policy from that cache, and a
            // cold CLI process starts with defaults (everything off).
            registry
                .chat
                .update_profile_chat_settings(chat_cfg.clone())
                .await;
            let mut targets = Vec::new();
            if chat_cfg.twitch_send_enabled {
                targets.push(ChatPlatform::Twitch);
            }
            if chat_cfg.youtube_send_enabled && !chat_cfg.youtube_use_api_key {
                targets.push(ChatPlatform::YouTube);
            }
            if chat_cfg.trovo_send_enabled {
                targets.push(ChatPlatform::Trovo);
            }

            if targets.is_empty() {
                return Err(CliError::Argument(
                    "no chat platforms are enabled for sending in the active profile".into(),
                ));
            }

            let pii_policy = active_profile
                .as_ref()
                .map(|p| (p.pii_blocklist.clone(), p.pii_fuzzy));

            let results = registry
                .chat
                .send_message(trimmed, &targets, pii_policy, &registry.safety)
                .await;
            let out_results: Vec<ChatSendResult> = results
                .into_iter()
                .map(|(platform, result)| match result {
                    Ok(()) => ChatSendResult {
                        platform,
                        success: true,
                        error: None,
                        error_code: None,
                    },
                    Err(err) => ChatSendResult {
                        platform,
                        success: false,
                        error: Some(err.to_string()),
                        error_code: Some(err.kind().to_string()),
                    },
                })
                .collect();
            out.emit(&out_results)?;
            Ok(())
        }
        ChatCmd::Connect {
            platform,
            channel,
            channel_id,
            oauth_from,
            api_key_from,
            session_token_from,
            use_api_key,
        } => {
            let oauth =
                crate::secret_input::read_optional_secret(oauth_from, "OAuth access token")?;
            let api_key =
                crate::secret_input::read_optional_secret(api_key_from, "YouTube API key")?;
            let session_token = crate::secret_input::read_optional_secret(
                session_token_from,
                "TikTok session token",
            )?;
            let p = parse_platform(&platform)?;
            let config = build_connect_config(
                p,
                channel,
                channel_id,
                oauth,
                api_key,
                session_token,
                use_api_key,
                registry.oauth.get_config().await.get_trovo_client_id(),
            )?;
            registry.chat.connect(config).await?;
            out.emit(&serde_json::json!({ "platform": p, "connected": true }))?;
            Ok(())
        }
        ChatCmd::Retry {
            platform,
            channel,
            channel_id,
            oauth_from,
            api_key_from,
            session_token_from,
            use_api_key,
        } => {
            let oauth =
                crate::secret_input::read_optional_secret(oauth_from, "OAuth access token")?;
            let api_key =
                crate::secret_input::read_optional_secret(api_key_from, "YouTube API key")?;
            let session_token = crate::secret_input::read_optional_secret(
                session_token_from,
                "TikTok session token",
            )?;
            let p = parse_platform(&platform)?;
            // Disconnect first; "not connected" is acceptable for retry.
            if let Err(err) = registry.chat.disconnect(p).await {
                if err.kind() != "chat_platform_not_connected" {
                    return Err(err.into());
                }
            }
            let config = build_connect_config(
                p,
                channel,
                channel_id,
                oauth,
                api_key,
                session_token,
                use_api_key,
                registry.oauth.get_config().await.get_trovo_client_id(),
            )?;
            registry.chat.connect(config).await?;
            out.emit(&serde_json::json!({ "platform": p, "reconnected": true }))?;
            Ok(())
        }
        ChatCmd::Disconnect { platform } => {
            let p = parse_platform(&platform)?;
            registry.chat.disconnect(p).await?;
            out.emit(&serde_json::json!({ "platform": p, "disconnected": true }))?;
            Ok(())
        }
        ChatCmd::DisconnectAll => {
            registry.chat.disconnect_all("user_requested").await?;
            out.emit(&serde_json::json!({ "disconnectedAll": true }))?;
            Ok(())
        }
        ChatCmd::Connected => {
            let connected = registry.chat.is_any_connected().await;
            out.emit(&serde_json::json!({ "connected": connected }))?;
            Ok(())
        }
        ChatCmd::LogStatus => {
            let start_ms = registry.chat.log_session_start_ms();
            out.emit(&serde_json::json!({
                "active": start_ms.is_some(),
                "startedAt": start_ms,
            }))?;
            Ok(())
        }
        ChatCmd::PlatformStatus { platform } => {
            let p = parse_platform(&platform)?;
            let status = registry.chat.get_platform_status(p).await;
            out.emit(&status)?;
            Ok(())
        }
        ChatCmd::ExportLog { path } => {
            // Destination policy mirrors `system logs-export`: data dir
            // or home only — chat logs are sensitive (usernames,
            // timestamps) and must not be writable to arbitrary paths.
            let home = dirs_next::home_dir();
            let mut allowed: Vec<&std::path::Path> = vec![registry.data_dir.as_path()];
            if let Some(h) = home.as_deref() {
                allowed.push(h);
            }
            let path = spiritstream_core::services::validate_path_within_any(&path, &allowed)?;
            // Replicates `POST /api/v1/chat/log/export`: flush, then read every
            // chatlog_<hour>.jsonl line whose timestamp lies in the active
            // session window, write to `<path>`.
            use chrono::{Local, TimeZone};
            use spiritstream_core::models::ChatMessage;
            use std::fs::File;
            use std::io::{BufRead, BufReader, BufWriter, Write};

            let start_ms = registry
                .chat
                .log_session_start_ms()
                .ok_or_else(|| CliError::Argument("no active chat session to export".into()))?;
            registry.chat.flush_chat_logs().await?;
            let end_ms = Local::now().timestamp_millis();
            let start_dt = Local
                .timestamp_millis_opt(start_ms)
                .single()
                .unwrap_or_else(Local::now);
            let end_dt = Local
                .timestamp_millis_opt(end_ms)
                .single()
                .unwrap_or_else(Local::now);

            // Reuse the same hour-key builder the HTTP path uses.
            let mut keys: Vec<String> = Vec::new();
            let mut cursor = start_dt;
            while cursor <= end_dt {
                keys.push(cursor.format("%Y-%m-%d_%H").to_string());
                cursor += chrono::Duration::hours(1);
            }

            let mut writer = BufWriter::new(
                File::create(&path)
                    .map_err(|e| CliError::Io(format!("create {}: {e}", path.display())))?,
            );
            for key in keys {
                let src = registry.log_dir.join(format!("chatlog_{}.jsonl", key));
                if !src.exists() {
                    continue;
                }
                let f = File::open(&src)
                    .map_err(|e| CliError::Io(format!("read {}: {e}", src.display())))?;
                for line in BufReader::new(f).lines() {
                    let line = line.map_err(|e| CliError::Io(format!("read chat log: {e}")))?;
                    if line.trim().is_empty() {
                        continue;
                    }
                    if let Ok(m) = serde_json::from_str::<ChatMessage>(&line) {
                        if m.timestamp >= start_ms && m.timestamp <= end_ms {
                            writer
                                .write_all(line.as_bytes())
                                .map_err(|e| CliError::Io(format!("write export: {e}")))?;
                            writer
                                .write_all(b"\n")
                                .map_err(|e| CliError::Io(format!("write export: {e}")))?;
                        }
                    }
                }
            }
            writer
                .flush()
                .map_err(|e| CliError::Io(format!("flush export: {e}")))?;
            out.emit(&serde_json::json!({ "exported": true, "path": path }))?;
            Ok(())
        }
        ChatCmd::Search { query, limit } => {
            use chrono::{Local, TimeZone};
            use spiritstream_core::models::ChatMessage;
            use std::fs::File;
            use std::io::{BufRead, BufReader};

            let limit = limit.unwrap_or(500);
            let start_ms = registry
                .chat
                .log_session_start_ms()
                .ok_or_else(|| CliError::Argument("no active chat session to search".into()))?;
            let q = query.trim().to_lowercase();
            if q.is_empty() {
                out.emit(&Vec::<ChatMessage>::new())?;
                return Ok(());
            }
            let end_ms = Local::now().timestamp_millis();
            let start_dt = Local
                .timestamp_millis_opt(start_ms)
                .single()
                .unwrap_or_else(Local::now);
            let end_dt = Local
                .timestamp_millis_opt(end_ms)
                .single()
                .unwrap_or_else(Local::now);
            let mut keys: Vec<String> = Vec::new();
            let mut cursor = start_dt;
            while cursor <= end_dt {
                keys.push(cursor.format("%Y-%m-%d_%H").to_string());
                cursor += chrono::Duration::hours(1);
            }
            let mut matches: Vec<ChatMessage> = Vec::new();
            for key in keys {
                if matches.len() >= limit {
                    break;
                }
                let src = registry.log_dir.join(format!("chatlog_{}.jsonl", key));
                if !src.exists() {
                    continue;
                }
                let f = File::open(&src)
                    .map_err(|e| CliError::Io(format!("read {}: {e}", src.display())))?;
                for line in BufReader::new(f).lines() {
                    if matches.len() >= limit {
                        break;
                    }
                    let line = line.map_err(|e| CliError::Io(format!("read chat log: {e}")))?;
                    if line.trim().is_empty() {
                        continue;
                    }
                    let m = match serde_json::from_str::<ChatMessage>(&line) {
                        Ok(m) => m,
                        Err(_) => continue,
                    };
                    if m.timestamp < start_ms || m.timestamp > end_ms {
                        continue;
                    }
                    if m.username.to_lowercase().contains(&q)
                        || m.message.to_lowercase().contains(&q)
                    {
                        matches.push(m);
                    }
                }
            }
            out.emit(&matches)?;
            Ok(())
        }
    }
}
