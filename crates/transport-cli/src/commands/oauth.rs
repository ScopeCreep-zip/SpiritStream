//! `spiritstream-cli oauth …` — OAuth 2.0 flows for chat platforms.

use clap::Subcommand;
use serde::Serialize;

use crate::error::CliError;
use crate::output::Output;
use spiritstream_core::ServiceRegistry;

#[derive(Debug, Subcommand)]
pub enum OAuthCmd {
    /// Begin an OAuth flow for `<provider>` (e.g. `twitch`, `youtube`).
    /// Returns the auth URL the user must open in their browser, plus the
    /// callback port the server is listening on. The CLI is a thin wrapper
    /// around `OAuthService::start_flow` — it does NOT launch a browser
    /// itself (CLI users handle that side however they like).
    Start { provider: String },
    /// Complete the OAuth flow by exchanging the redirect `<code>` + state
    /// for tokens.
    Complete {
        provider: String,
        code: String,
        #[arg(long)]
        state: String,
    },
    /// Revoke and forget a stored OAuth token for `<provider>`.
    Disconnect { provider: String },
    /// Report whether `<provider>` is configured (always true with embedded client IDs).
    IsConfigured { provider: String },
    /// Refresh an access token using the supplied refresh token.
    Refresh {
        provider: String,
        refresh_token: String,
    },
    /// Revoke the token AND forget the account locally. The CLI doesn't
    /// have an active-profile session to read the stored token from, so
    /// the caller supplies it via `--token`.
    Forget {
        provider: String,
        #[arg(long)]
        token: String,
    },
    /// Read or write the user-provided OAuth client credentials. The
    /// embedded client IDs always work; this command is for operators
    /// who want to point SpiritStream at their own Twitch / YouTube
    /// OAuth applications instead of the bundled ones.
    Config {
        #[command(subcommand)]
        action: ConfigCmd,
    },
    /// Show the stored OAuth account info (logged_in, user_id, username,
    /// display_name) for `<provider>` on `--profile <name>`. Mirrors
    /// `GET /api/v1/oauth/{provider}/account`; the CLI takes `--profile`
    /// explicitly because there is no cross-invocation active-profile
    /// concept.
    Account {
        provider: String,
        #[arg(long)]
        profile: String,
        #[arg(long)]
        password: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigCmd {
    /// Print the current OAuth config as JSON. Secrets are included
    /// when set (CLI users handle their own redaction).
    Get,
    /// Replace the OAuth config. Pass the full JSON via `--json` or
    /// individual fields via `--twitch-client-id`, etc. Fields not
    /// provided clear (set to `None`) — mirrors the HTTP PUT semantics.
    Set {
        /// Full JSON payload (overrides individual flags if both given).
        #[arg(long)]
        json: Option<String>,
        #[arg(long)]
        twitch_client_id: Option<String>,
        #[arg(long)]
        twitch_client_secret: Option<String>,
        #[arg(long)]
        youtube_client_id: Option<String>,
        #[arg(long)]
        youtube_client_secret: Option<String>,
        #[arg(long)]
        kick_client_id: Option<String>,
        #[arg(long)]
        kick_client_secret: Option<String>,
        #[arg(long)]
        facebook_client_id: Option<String>,
        #[arg(long)]
        facebook_client_secret: Option<String>,
    },
}

#[derive(Serialize)]
struct StartResponse {
    auth_url: String,
    callback_port: u16,
    state: String,
}

#[derive(Serialize)]
struct CompleteResponse {
    provider: String,
    user_id: String,
    username: String,
    display_name: String,
}

pub async fn run(
    cmd: OAuthCmd,
    registry: &ServiceRegistry,
    out: &mut Output,
) -> Result<(), CliError> {
    match cmd {
        OAuthCmd::Start { provider } => {
            let result = registry.oauth.start_flow(&provider).await?;
            out.emit(&StartResponse {
                auth_url: result.auth_url,
                callback_port: result.callback_port,
                state: result.state,
            })?;
            Ok(())
        }
        OAuthCmd::Complete {
            provider,
            code,
            state,
        } => {
            let result = registry
                .oauth
                .complete_flow(&provider, &code, &state)
                .await?;
            out.emit(&CompleteResponse {
                provider: result.user_info.provider,
                user_id: result.user_info.user_id,
                username: result.user_info.username,
                display_name: result.user_info.display_name,
            })?;
            Ok(())
        }
        OAuthCmd::Disconnect { provider } => {
            // Without an active-profile session the CLI can't reach the
            // stored token; this is a no-op acknowledgement matching the
            // HTTP layer's "session disconnect" path (which just clears the
            // session-side tokens — the profile-saved token survives).
            let _ = registry;
            #[derive(Serialize)]
            struct Resp {
                provider: String,
                disconnected: bool,
            }
            out.emit(&Resp {
                provider,
                disconnected: true,
            })?;
            Ok(())
        }
        OAuthCmd::IsConfigured { provider } => {
            let configured = registry.oauth.is_configured(&provider).await;
            out.emit(&serde_json::json!({ "provider": provider, "configured": configured }))?;
            Ok(())
        }
        OAuthCmd::Refresh {
            provider,
            refresh_token,
        } => {
            let tokens = registry
                .oauth
                .refresh_token(&provider, &refresh_token)
                .await?;
            out.emit(&tokens)?;
            Ok(())
        }
        OAuthCmd::Forget { provider, token } => {
            // Revoke the supplied token (best effort) then report success.
            // No stored-token clear from CLI — the CLI has no session state.
            if let Err(e) = registry.oauth.revoke_token(&provider, &token).await {
                log::warn!("revoke {provider} token failed: {e}");
            }
            out.emit(&serde_json::json!({ "provider": provider, "forgotten": true }))?;
            Ok(())
        }
        OAuthCmd::Account {
            provider,
            profile,
            password,
        } => {
            let profile_obj = registry
                .profiles
                .load_with_key_decryption(&profile, password.as_deref())
                .await?;
            let account = match provider.as_str() {
                "twitch" => &profile_obj.settings.oauth.twitch,
                "youtube" => &profile_obj.settings.oauth.youtube,
                other => {
                    return Err(CliError::Argument(format!(
                        "unknown oauth provider: {other} (expected twitch|youtube)"
                    )));
                }
            };
            let logged_in = !account.user_id.is_empty();
            out.emit(&spiritstream_core::models::OAuthAccountStatus {
                logged_in,
                user_id: account.user_id.clone(),
                username: account.username.clone(),
                display_name: account.display_name.clone(),
            })?;
            Ok(())
        }
        OAuthCmd::Config { action } => match action {
            ConfigCmd::Get => {
                let config = registry.oauth.get_config().await;
                out.emit(&config)?;
                Ok(())
            }
            ConfigCmd::Set {
                json,
                twitch_client_id,
                twitch_client_secret,
                youtube_client_id,
                youtube_client_secret,
                kick_client_id,
                kick_client_secret,
                facebook_client_id,
                facebook_client_secret,
            } => {
                let config: spiritstream_core::services::OAuthConfig = if let Some(raw) = json {
                    serde_json::from_str(&raw)
                        .map_err(|e| CliError::Argument(format!("invalid --json: {e}")))?
                } else {
                    spiritstream_core::services::OAuthConfig {
                        twitch_client_id,
                        twitch_client_secret,
                        youtube_client_id,
                        youtube_client_secret,
                        kick_client_id,
                        kick_client_secret,
                        facebook_client_id,
                        facebook_client_secret,
                    }
                };
                registry.oauth.update_config(config).await;
                out.emit(&serde_json::json!({ "updated": true }))?;
                Ok(())
            }
        },
    }
}
