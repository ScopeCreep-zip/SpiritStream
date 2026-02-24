use serde_json::{json, Value};

use crate::events::ServerEvent;
use crate::state::{clear_profile_oauth_account, get_active_profile_name, get_active_profile_settings, update_profile_oauth_account, AppState};
use crate::util::get_arg;
use spiritstream_server::services::{EventSink, OAuthCallback, OAuthCallbackServer, OAuthConfig};

pub(crate) async fn handle(state: &AppState, command: &str, payload: &Value) -> Result<Value, String> {
    match command {
        "oauth_is_configured" => {
            let provider: String = get_arg(payload, "provider")?;
            // Always configured now with embedded client IDs
            let configured = state.oauth_service.is_configured(&provider).await;
            Ok(json!(configured))
        }
        "oauth_start_flow" => {
            let provider: String = get_arg(payload, "provider")?;
            if get_active_profile_name(state).await.is_none() {
                return Err("No active profile loaded".to_string());
            }
            let result = state.oauth_service.start_flow(&provider).await?;

            // Start the local callback server to receive the OAuth redirect
            let (callback_server, mut callback_rx) =
                OAuthCallbackServer::start(result.callback_port).await.map_err(|e| {
                    format!("Failed to start OAuth callback server: {e}")
                })?;

            let oauth_service = state.oauth_service.clone();
            let state_clone = state.clone();
            let provider_name = provider.clone();

            tokio::spawn(async move {
                let timeout = tokio::time::sleep(std::time::Duration::from_secs(180));
                tokio::pin!(timeout);

                let callback = tokio::select! {
                    res = &mut callback_rx => res.ok(),
                    _ = &mut timeout => None,
                };

                match callback {
                    Some(OAuthCallback::Success { code, state }) => {
                        match oauth_service.complete_flow(&provider_name, &code, &state).await {
                            Ok(result) => {
                                let now = chrono::Utc::now().timestamp();
                                let expires_at = result.tokens.expires_in.map(|e| now + e as i64).unwrap_or(0);

                                match update_profile_oauth_account(
                                    &state_clone,
                                    &provider_name,
                                    result.tokens.access_token.clone(),
                                    result.tokens.refresh_token.clone(),
                                    expires_at,
                                    &result.user_info,
                                )
                                .await
                                {
                                    Ok(()) => {
                                        state_clone.event_bus.emit("oauth_complete", json!(result.user_info));
                                    }
                                    Err(err) => {
                                        log::error!("Failed to save OAuth profile settings: {err}");
                                    }
                                }
                            }
                            Err(err) => {
                                log::error!("OAuth completion failed for {provider_name}: {err}");
                            }
                        }
                    }
                    Some(OAuthCallback::ImplicitSuccess { access_token, state: _state }) => {
                        // Implicit flow (legacy) -- token received directly, no exchange needed
                        log::info!("Implicit OAuth flow completed for {provider_name}");

                        // Fetch user info using the access token
                        let user_info_result = match provider_name.as_str() {
                            "twitch" => oauth_service.fetch_twitch_user(&access_token).await.map(|u| {
                                spiritstream_server::services::OAuthUserInfo {
                                    provider: "twitch".to_string(),
                                    user_id: u.id,
                                    username: u.login,
                                    display_name: u.display_name,
                                }
                            }),
                            _ => Err("Implicit flow not supported for this provider".to_string()),
                        };

                        match user_info_result {
                            Ok(user_info) => {
                                // Implicit flow tokens don't have refresh tokens or expiry
                                match update_profile_oauth_account(
                                    &state_clone,
                                    &provider_name,
                                    access_token,
                                    None,
                                    0,
                                    &user_info,
                                )
                                .await
                                {
                                    Ok(()) => {
                                        state_clone.event_bus.emit("oauth_complete", json!(user_info));
                                    }
                                    Err(err) => {
                                        log::error!("Failed to save OAuth profile settings: {err}");
                                    }
                                }
                            }
                            Err(err) => {
                                log::error!("Failed to fetch user info for {provider_name}: {err}");
                            }
                        }
                    }
                    Some(OAuthCallback::Error { error, description }) => {
                        if let Some(description) = description {
                            log::warn!("OAuth callback error for {provider_name}: {error} ({description})");
                        } else {
                            log::warn!("OAuth callback error for {provider_name}: {error}");
                        }
                    }
                    None => {
                        log::warn!("OAuth callback timed out for {provider_name}");
                    }
                }

                callback_server.shutdown();
            });

            // Open the auth URL in the default browser
            if let Err(e) = opener::open(&result.auth_url) {
                log::warn!("Failed to open browser: {}. URL: {}", e, result.auth_url);
            }

            Ok(json!(result))
        }
        "oauth_complete_flow" => {
            // Complete OAuth flow: exchange code, fetch user info, store tokens
            let provider: String = get_arg(payload, "provider")?;
            let code: String = get_arg(payload, "code")?;
            let oauth_state: String = get_arg(payload, "state")?;
            if get_active_profile_name(state).await.is_none() {
                return Err("No active profile loaded".to_string());
            }

            // Complete the flow (exchange code + fetch user info)
            let result = state.oauth_service.complete_flow(&provider, &code, &oauth_state).await?;

            // Store tokens and user info in profile
            let now = chrono::Utc::now().timestamp();
            let expires_at = result.tokens.expires_in.map(|e| now + e as i64).unwrap_or(0);
            update_profile_oauth_account(
                state,
                &provider,
                result.tokens.access_token.clone(),
                result.tokens.refresh_token.clone(),
                expires_at,
                &result.user_info,
            )
            .await?;

            // Emit event for frontend
            state.event_bus.sender.send(ServerEvent {
                event: "oauth_complete".to_string(),
                payload: json!(result.user_info),
            }).ok();

            Ok(json!(result.user_info))
        }
        "oauth_get_account" => {
            // Get stored OAuth account info for a provider
            let provider: String = get_arg(payload, "provider")?;
            let profile_settings = get_active_profile_settings(state).await;

            let account = match provider.as_str() {
                "twitch" => {
                    if let Some(settings) = profile_settings.as_ref() {
                        if !settings.oauth.twitch.username.is_empty() {
                            json!({
                                "loggedIn": true,
                                "userId": settings.oauth.twitch.user_id,
                                "username": settings.oauth.twitch.username,
                                "displayName": settings.oauth.twitch.display_name
                            })
                        } else {
                            json!({ "loggedIn": false })
                        }
                    } else {
                        json!({
                            "loggedIn": false
                        })
                    }
                }
                "youtube" => {
                    if let Some(settings) = profile_settings.as_ref() {
                        if !settings.oauth.youtube.user_id.is_empty() {
                            json!({
                                "loggedIn": true,
                                "userId": settings.oauth.youtube.user_id,
                                "username": settings.oauth.youtube.username,
                                "displayName": settings.oauth.youtube.display_name
                            })
                        } else {
                            json!({ "loggedIn": false })
                        }
                    } else {
                        json!({
                            "loggedIn": false
                        })
                    }
                }
                _ => json!({ "loggedIn": false })
            };

            Ok(account)
        }
        "oauth_disconnect" => {
            // Clear OAuth tokens but don't revoke (user might reconnect)
            let provider: String = get_arg(payload, "provider")?;
            clear_profile_oauth_account(state, &provider).await?;
            Ok(Value::Null)
        }
        "oauth_forget" => {
            // Revoke tokens AND clear from settings
            let provider: String = get_arg(payload, "provider")?;
            let profile_settings = get_active_profile_settings(state)
                .await
                .ok_or_else(|| "No active profile loaded".to_string())?;

            // Try to revoke the token (best effort)
            let token = match provider.as_str() {
                "twitch" => profile_settings.oauth.twitch.access_token,
                "youtube" => profile_settings.oauth.youtube.access_token,
                _ => return Err(format!("Unknown provider: {}", provider)),
            };

            if !token.is_empty() {
                if let Err(e) = state.oauth_service.revoke_token(&provider, token.as_str()).await {
                    log::warn!("Failed to revoke {} token: {}", provider, e);
                }
            }

            // Clear from settings (same as disconnect + clear channel config)
            clear_profile_oauth_account(state, &provider).await?;
            Ok(Value::Null)
        }
        "oauth_refresh_token" => {
            let provider: String = get_arg(payload, "provider")?;
            let refresh_token: String = get_arg(payload, "refreshToken")?;
            let tokens = state.oauth_service.refresh_token(&provider, &refresh_token).await?;
            Ok(json!(tokens))
        }
        "oauth_get_config" => {
            // Always configured now with embedded client IDs
            Ok(json!({
                "twitchConfigured": true,
                "youtubeConfigured": true
            }))
        }
        "oauth_set_config" => {
            // Still allow users to override with their own credentials if desired
            let config: OAuthConfig = get_arg(payload, "config")?;
            state.oauth_service.update_config(config).await;
            Ok(Value::Null)
        }
        _ => Err(format!("Unknown oauth command: {command}")),
    }
}
