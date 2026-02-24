use chrono::{Local, TimeZone};
use serde_json::{json, Value};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};

use crate::chat_lifecycle::{connect_trovo_chat, connect_twitch_chat, connect_youtube_chat_with_retry};
use crate::state::{apply_and_persist_oauth_refresh, ensure_fresh_oauth_token, get_active_profile_settings, AppState, FreshOAuthToken};
use crate::util::{build_hour_keys, get_arg, get_opt_arg};
use spiritstream_server::models::{ChatConfig, ChatCredentials, ChatMessage, ChatPlatform, ChatSendResult, TwitchAuth, YouTubeAuth};
use spiritstream_server::services::EventSink;

pub(crate) async fn handle(state: &AppState, command: &str, payload: &Value) -> Result<Value, String> {
    match command {
        "connect_chat" => {
            let mut config: ChatConfig = get_arg(payload, "config")?;

            // Enrich credentials with stored OAuth tokens when frontend sends empty placeholders,
            // and refresh expired tokens automatically.
            let mut profile_settings = get_active_profile_settings(state)
                .await
                .ok_or_else(|| "No active profile loaded".to_string())?;
            config.credentials = match config.credentials {
                ChatCredentials::Twitch { channel, auth } => {
                    let enriched_auth = match auth {
                        Some(TwitchAuth::AppOAuth { access_token, refresh_token, expires_at })
                            if access_token.is_empty() =>
                        {
                            if profile_settings.oauth.twitch.access_token.is_empty() {
                                return Err("No Twitch OAuth token stored. Please login with Twitch first.".to_string());
                            }
                            // Refresh token if expired
                            let fresh = ensure_fresh_oauth_token(
                                "twitch",
                                &profile_settings.oauth.twitch.access_token,
                                &profile_settings.oauth.twitch.refresh_token,
                                profile_settings.oauth.twitch.expires_at,
                                &state.oauth_service,
                            ).await.unwrap_or_else(|e| {
                                log::warn!("Twitch token refresh failed: {e}");
                                FreshOAuthToken {
                                    access_token: profile_settings.oauth.twitch.access_token.clone(),
                                    refresh_token: None,
                                    expires_at: profile_settings.oauth.twitch.expires_at,
                                    refreshed: false,
                                }
                            });
                            let token = apply_and_persist_oauth_refresh(state, "twitch", &fresh, &mut profile_settings).await;
                            Some(TwitchAuth::AppOAuth {
                                access_token: token,
                                refresh_token: if refresh_token.is_none() {
                                    Some(profile_settings.oauth.twitch.refresh_token.clone())
                                        .filter(|s| !s.is_empty())
                                } else {
                                    refresh_token
                                },
                                expires_at: if expires_at.is_none() && profile_settings.oauth.twitch.expires_at > 0 {
                                    Some(profile_settings.oauth.twitch.expires_at)
                                } else {
                                    expires_at
                                },
                            })
                        }
                        other => other,
                    };
                    ChatCredentials::Twitch { channel, auth: enriched_auth }
                }
                ChatCredentials::YouTube { channel_id, auth } => {
                    let enriched_auth = match auth {
                        YouTubeAuth::AppOAuth { access_token, refresh_token, expires_at }
                            if access_token.is_empty() =>
                        {
                            if profile_settings.oauth.youtube.access_token.is_empty() {
                                return Err("No YouTube OAuth token stored. Please sign in with Google first.".to_string());
                            }
                            // Refresh token if expired
                            let fresh = ensure_fresh_oauth_token(
                                "youtube",
                                &profile_settings.oauth.youtube.access_token,
                                &profile_settings.oauth.youtube.refresh_token,
                                profile_settings.oauth.youtube.expires_at,
                                &state.oauth_service,
                            ).await.unwrap_or_else(|e| {
                                log::warn!("YouTube token refresh failed: {e}");
                                FreshOAuthToken {
                                    access_token: profile_settings.oauth.youtube.access_token.clone(),
                                    refresh_token: None,
                                    expires_at: profile_settings.oauth.youtube.expires_at,
                                    refreshed: false,
                                }
                            });
                            let token = apply_and_persist_oauth_refresh(state, "youtube", &fresh, &mut profile_settings).await;
                            YouTubeAuth::AppOAuth {
                                access_token: token,
                                refresh_token: if refresh_token.is_none() {
                                    Some(profile_settings.oauth.youtube.refresh_token.clone())
                                        .filter(|s| !s.is_empty())
                                } else {
                                    refresh_token
                                },
                                expires_at: if expires_at.is_none() && profile_settings.oauth.youtube.expires_at > 0 {
                                    Some(profile_settings.oauth.youtube.expires_at)
                                } else {
                                    expires_at
                                },
                            }
                        }
                        other => other,
                    };
                    ChatCredentials::YouTube { channel_id, auth: enriched_auth }
                }
                other => other,
            };

            state.chat_manager.connect(config).await?;
            Ok(Value::Null)
        }
        "send_chat_message" => {
            let message: String = get_arg(payload, "message")?;
            let trimmed = message.trim().to_string();
            if trimmed.is_empty() {
                return Err("Message cannot be empty".to_string());
            }

            let mut targets = Vec::new();
            let chat_settings = state.chat_manager.profile_chat_settings().await;
            if chat_settings.twitch_send_enabled {
                targets.push(ChatPlatform::Twitch);
            }
            if chat_settings.youtube_send_enabled && !chat_settings.youtube_use_api_key {
                targets.push(ChatPlatform::YouTube);
            }
            if chat_settings.trovo_send_enabled {
                targets.push(ChatPlatform::Trovo);
            }
            if chat_settings.stripchat_send_enabled {
                targets.push(ChatPlatform::Stripchat);
            }

            if targets.is_empty() {
                return Err("No chat platforms are enabled for sending".to_string());
            }

            let results = state.chat_manager.send_message(trimmed.clone(), &targets).await;
            let mut send_results: Vec<ChatSendResult> = Vec::new();
            let mut successes: Vec<ChatPlatform> = Vec::new();

            for (platform, result) in results {
                match result {
                    Ok(()) => {
                        successes.push(platform);
                        send_results.push(ChatSendResult {
                            platform,
                            success: true,
                            error: None,
                        });
                    }
                    Err(err) => {
                        send_results.push(ChatSendResult {
                            platform,
                            success: false,
                            error: Some(err),
                        });
                    }
                }
            }

            if !successes.is_empty() {
                let outbound = ChatMessage::new_outbound(
                    successes,
                    "You".to_string(),
                    trimmed,
                );
                state.chat_manager.log_message(outbound.clone());
                if let Ok(payload) = serde_json::to_value(&outbound) {
                    state.event_bus.emit("chat_message", payload);
                }
            }

            Ok(json!(send_results))
        }
        "chat_export_log" => {
            let path: String = get_arg(payload, "path")?;
            let start_ms = state
                .chat_manager
                .log_session_start_ms()
                .ok_or_else(|| "No active chat session to export".to_string())?;

            // Flush any buffered log lines before exporting
            state.chat_manager.flush_chat_logs().await?;

            let end_ms = chrono::Local::now().timestamp_millis();
            let start_dt = Local
                .timestamp_millis_opt(start_ms)
                .single()
                .unwrap_or_else(Local::now);
            let end_dt = Local
                .timestamp_millis_opt(end_ms)
                .single()
                .unwrap_or_else(Local::now);

            let hour_keys = build_hour_keys(start_dt, end_dt);
            let mut writer = BufWriter::new(
                File::create(&path).map_err(|e| format!("Failed to create export file: {}", e))?,
            );

            for key in hour_keys {
                let src_path = state.log_dir.join(format!("chatlog_{}.jsonl", key));
                if !src_path.exists() {
                    continue;
                }

                let file = File::open(&src_path)
                    .map_err(|e| format!("Failed to read chat log {}: {}", src_path.display(), e))?;
                let reader = BufReader::new(file);
                for line in reader.lines() {
                    let line = line.map_err(|e| format!("Failed to read chat log: {}", e))?;
                    if line.trim().is_empty() {
                        continue;
                    }
                    if let Ok(message) = serde_json::from_str::<ChatMessage>(&line) {
                        if message.timestamp >= start_ms && message.timestamp <= end_ms {
                            writer
                                .write_all(line.as_bytes())
                                .map_err(|e| format!("Failed to write export file: {}", e))?;
                            writer
                                .write_all(b"\n")
                                .map_err(|e| format!("Failed to write export file: {}", e))?;
                        }
                    }
                }
            }

            writer
                .flush()
                .map_err(|e| format!("Failed to finalize export file: {}", e))?;

            Ok(Value::Null)
        }
        "chat_search_session" => {
            let query: String = get_arg(payload, "query")?;
            let limit: Option<usize> = get_opt_arg(payload, "limit")?;
            let limit = limit.unwrap_or(500);

            let start_ms = state
                .chat_manager
                .log_session_start_ms()
                .ok_or_else(|| "No active chat session to search".to_string())?;

            let query = query.trim().to_lowercase();
            if query.is_empty() {
                return Ok(json!([]));
            }

            let end_ms = chrono::Local::now().timestamp_millis();
            let start_dt = Local
                .timestamp_millis_opt(start_ms)
                .single()
                .unwrap_or_else(Local::now);
            let end_dt = Local
                .timestamp_millis_opt(end_ms)
                .single()
                .unwrap_or_else(Local::now);

            let hour_keys = build_hour_keys(start_dt, end_dt);
            let mut matches: Vec<ChatMessage> = Vec::new();

            for key in hour_keys {
                if matches.len() >= limit {
                    break;
                }

                let src_path = state.log_dir.join(format!("chatlog_{}.jsonl", key));
                if !src_path.exists() {
                    continue;
                }

                let file = File::open(&src_path)
                    .map_err(|e| format!("Failed to read chat log {}: {}", src_path.display(), e))?;
                let reader = BufReader::new(file);
                for line in reader.lines() {
                    if matches.len() >= limit {
                        break;
                    }
                    let line = line.map_err(|e| format!("Failed to read chat log: {}", e))?;
                    if line.trim().is_empty() {
                        continue;
                    }
                    let message = match serde_json::from_str::<ChatMessage>(&line) {
                        Ok(m) => m,
                        Err(_) => continue,
                    };
                    if message.timestamp < start_ms || message.timestamp > end_ms {
                        continue;
                    }

                    let username = message.username.to_lowercase();
                    let text = message.message.to_lowercase();
                    if username.contains(&query) || text.contains(&query) {
                        matches.push(message);
                    }
                }
            }

            Ok(json!(matches))
        }
        "disconnect_chat" => {
            let platform: ChatPlatform = get_arg(payload, "platform")?;
            state.chat_manager.disconnect(platform).await?;
            Ok(Value::Null)
        }
        "retry_chat_connection" => {
            let platform: ChatPlatform = get_arg(payload, "platform")?;
            let chat_settings = state.chat_manager.profile_chat_settings().await;
            let profile_settings = get_active_profile_settings(state)
                .await
                .ok_or_else(|| "No active profile loaded".to_string())?;

            if state.ffmpeg_handler.active_count() == 0 {
                return Err("Cannot reconnect chat when no stream is active".to_string());
            }

            match platform {
                ChatPlatform::Twitch => {
                    if chat_settings.twitch_channel.is_empty() {
                        return Err("Twitch chat is not configured".to_string());
                    }
                    connect_twitch_chat(&state.chat_manager, &chat_settings, &profile_settings, &state.event_bus).await;
                }
                ChatPlatform::Trovo => {
                    if chat_settings.trovo_channel_id.is_empty() {
                        return Err("Trovo chat is not configured".to_string());
                    }
                    connect_trovo_chat(&state.chat_manager, &chat_settings, &state.event_bus).await;
                }
                ChatPlatform::YouTube => {
                    let has_oauth = !chat_settings.youtube_use_api_key
                        && !profile_settings.oauth.youtube.access_token.is_empty();
                    let has_api_key = chat_settings.youtube_use_api_key
                        && !chat_settings.youtube_api_key.is_empty();
                    if chat_settings.youtube_channel_id.is_empty() || (!has_oauth && !has_api_key) {
                        return Err("YouTube chat is not configured".to_string());
                    }
                    tokio::spawn(connect_youtube_chat_with_retry(state.clone()));
                }
                _ => return Err("Retry not supported for this platform".to_string()),
            }

            Ok(Value::Null)
        }
        "disconnect_all_chat" => {
            state.chat_manager.disconnect_all().await?;
            Ok(Value::Null)
        }
        "get_chat_status" => {
            let status = state.chat_manager.get_status().await;
            Ok(json!(status))
        }
        "chat_get_log_status" => {
            let start_ms = state.chat_manager.log_session_start_ms();
            Ok(json!({
                "active": start_ms.is_some(),
                "startedAt": start_ms
            }))
        }
        "get_platform_chat_status" => {
            let platform: ChatPlatform = get_arg(payload, "platform")?;
            let status = state.chat_manager.get_platform_status(platform).await;
            Ok(json!(status))
        }
        "is_chat_connected" => {
            let connected = state.chat_manager.is_any_connected().await;
            Ok(json!(connected))
        }
        _ => Err(format!("Unknown chat command: {command}")),
    }
}
