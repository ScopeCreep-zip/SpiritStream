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
    /// Disconnect `<provider>` on a profile: best-effort upstream token
    /// revocation, then clear the stored account from the profile and
    /// save it.
    Disconnect {
        provider: String,
        /// Profile holding the account.
        #[arg(long)]
        profile: String,
        /// Password for encrypted profiles.
        #[arg(long = "password-from", value_enum)]
        password_from: Option<crate::secret_input::SecretSource>,
    },
    /// Report whether `<provider>` has REAL credentials in this build
    /// (env override or release-embedded — placeholders report false).
    IsConfigured { provider: String },
    /// Sign in via the Device Code Flow (Twitch's mandated desktop
    /// path): prints a short code + verification URL, polls until you
    /// approve in any browser, then persists the account to
    /// `--profile`. No loopback server, no redirect URI.
    Device {
        provider: String,
        /// Profile to persist the signed-in account onto.
        #[arg(long)]
        profile: String,
        /// Password source for encrypted profiles.
        #[arg(long = "password-from", value_enum)]
        password_from: Option<crate::secret_input::SecretSource>,
    },
    /// Refresh `<provider>`'s access token using the refresh token
    /// STORED on `--profile` (by-reference — the secret never rides
    /// argv), and persist the rotated tokens back to the profile.
    Refresh {
        provider: String,
        /// Profile holding the account.
        #[arg(long)]
        profile: String,
        /// Password source for encrypted profiles.
        #[arg(long = "password-from", value_enum)]
        password_from: Option<crate::secret_input::SecretSource>,
    },
    /// Revoke a token the caller supplies (for tokens NOT stored on a
    /// profile — for stored accounts use `disconnect`). The token is
    /// read from stdin or an interactive prompt, never argv.
    Forget {
        provider: String,
        /// Source for the token to revoke.
        #[arg(long = "token-from", value_enum)]
        token_from: crate::secret_input::SecretSource,
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
        #[arg(long = "password-from", value_enum)]
        password_from: Option<crate::secret_input::SecretSource>,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigCmd {
    /// Print per-provider setup summaries as JSON (configured flag,
    /// needs_secret, override client id, registration URL). Secret
    /// values never print — `configured` already conveys their presence.
    Get,
    /// Replace the OAuth config. Pass the full JSON via `--json` or
    /// individual fields via `--twitch-client-id`, etc. Fields not
    /// provided clear (set to `None`) — mirrors the HTTP PUT semantics.
    Set(Box<ConfigSetArgs>),
    /// Store ONE provider's client credentials (the CLI mirror of the
    /// in-app "Set up sign-in" form; persists across restarts). Omit
    /// both values to clear the provider's override. The secret rides
    /// stdin/prompt/env via `--client-secret-from`, never argv.
    SetProvider {
        provider: String,
        #[arg(long)]
        client_id: Option<String>,
        #[arg(long = "client-secret-from", value_enum)]
        client_secret_from: Option<crate::secret_input::SecretSource>,
    },
}

#[derive(Debug, clap::Args)]
pub struct ConfigSetArgs {
    /// Full JSON payload (overrides individual flags if both given).
    #[arg(long)]
    pub json: Option<String>,
    #[arg(long)]
    pub twitch_client_id: Option<String>,
    #[arg(long)]
    pub twitch_client_secret: Option<String>,
    #[arg(long)]
    pub youtube_client_id: Option<String>,
    #[arg(long)]
    pub youtube_client_secret: Option<String>,
    #[arg(long)]
    pub kick_client_id: Option<String>,
    #[arg(long)]
    pub kick_client_secret: Option<String>,
    #[arg(long)]
    pub facebook_client_id: Option<String>,
    #[arg(long)]
    pub facebook_client_secret: Option<String>,
    #[arg(long)]
    pub trovo_client_id: Option<String>,
    #[arg(long)]
    pub trovo_client_secret: Option<String>,
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
        OAuthCmd::Disconnect {
            provider,
            profile,
            password_from,
        } => {
            let password =
                crate::secret_input::read_optional_secret(password_from, "Profile password")?;
            // Real disconnect (the old version printed
            // `"disconnected": true` while touching nothing): load the
            // profile, best-effort revoke the live token upstream, clear
            // the stored account, save. Revocation failure is loud but
            // non-fatal — the local clear is the part the user asked for.
            let mut p = registry
                .profiles
                .load_with_key_decryption(&profile, password.as_deref())
                .await?;
            let account = match provider.as_str() {
                "twitch" => &mut p.settings.oauth.twitch,
                "youtube" => &mut p.settings.oauth.youtube,
                "kick" => &mut p.settings.oauth.kick,
                "facebook" => &mut p.settings.oauth.facebook,
                "trovo" => &mut p.settings.oauth.trovo,
                other => {
                    return Err(CliError::Argument(format!(
                        "unknown oauth provider: {other} (expected twitch|youtube|kick|facebook|trovo)"
                    )));
                }
            };
            let mut revoked_upstream = false;
            if !account.access_token.is_empty() {
                match registry
                    .oauth
                    .revoke_token(&provider, &account.access_token)
                    .await
                {
                    Ok(()) => revoked_upstream = true,
                    Err(e) => log::warn!(
                        "upstream revoke for {provider} failed (clearing locally anyway): {e}"
                    ),
                }
            }
            *account = Default::default();
            registry
                .profiles
                .save_with_key_encryption(&p, password.as_deref())
                .await?;
            #[derive(Serialize)]
            #[serde(rename_all = "camelCase")]
            struct Resp {
                provider: String,
                disconnected: bool,
                revoked_upstream: bool,
            }
            out.emit(&Resp {
                provider,
                disconnected: true,
                revoked_upstream,
            })?;
            Ok(())
        }
        OAuthCmd::IsConfigured { provider } => {
            let configured = registry.oauth.is_configured(&provider).await;
            out.emit(&serde_json::json!({ "provider": provider, "configured": configured }))?;
            Ok(())
        }
        OAuthCmd::Device {
            provider,
            profile,
            password_from,
        } => {
            let password =
                crate::secret_input::read_optional_secret(password_from, "Profile password")?;
            // Load the profile FIRST so a bad name/password fails before
            // the user goes through the approve-in-browser dance.
            let mut p = registry
                .profiles
                .load_with_key_decryption(&profile, password.as_deref())
                .await?;

            let start = registry.oauth.start_device_flow(&provider).await?;
            eprintln!(
                "Visit {} and enter code: {}  (expires in {}s)",
                start.verification_uri, start.user_code, start.expires_in
            );
            let result = registry.oauth.poll_device_flow(&provider, &start).await?;

            let now = chrono::Utc::now().timestamp();
            let expires_at = result
                .tokens
                .expires_in
                .map(|s| now.saturating_add(i64::try_from(s).unwrap_or(i64::MAX)))
                .unwrap_or(0);
            let account = match provider.as_str() {
                "twitch" => &mut p.settings.oauth.twitch,
                "youtube" => &mut p.settings.oauth.youtube,
                "kick" => &mut p.settings.oauth.kick,
                "facebook" => &mut p.settings.oauth.facebook,
                "trovo" => &mut p.settings.oauth.trovo,
                other => {
                    return Err(CliError::Argument(format!(
                        "unknown oauth provider: {other} (expected twitch|youtube|kick|facebook|trovo)"
                    )));
                }
            };
            account.access_token = result.tokens.access_token.clone();
            if let Some(rt) = result.tokens.refresh_token.clone() {
                account.refresh_token = rt;
            }
            account.expires_at = expires_at;
            account.user_id = result.user_info.user_id.clone();
            account.username = result.user_info.username.clone();
            account.display_name = result.user_info.display_name.clone();
            registry
                .profiles
                .save_with_key_encryption(&p, password.as_deref())
                .await?;
            out.emit(&serde_json::json!({
                "provider": provider,
                "loggedIn": true,
                "username": result.user_info.username,
                "expiresAt": expires_at,
            }))?;
            Ok(())
        }
        OAuthCmd::Refresh {
            provider,
            profile,
            password_from,
        } => {
            let password =
                crate::secret_input::read_optional_secret(password_from, "Profile password")?;
            let mut p = registry
                .profiles
                .load_with_key_decryption(&profile, password.as_deref())
                .await?;
            let stored_refresh = match provider.as_str() {
                "twitch" => p.settings.oauth.twitch.refresh_token.clone(),
                "youtube" => p.settings.oauth.youtube.refresh_token.clone(),
                "kick" => p.settings.oauth.kick.refresh_token.clone(),
                "facebook" => p.settings.oauth.facebook.refresh_token.clone(),
                "trovo" => p.settings.oauth.trovo.refresh_token.clone(),
                other => {
                    return Err(CliError::Argument(format!(
                        "unknown oauth provider: {other} (expected twitch|youtube|kick|facebook|trovo)"
                    )));
                }
            };
            if stored_refresh.is_empty() {
                return Err(CliError::Argument(format!(
                    "profile '{profile}' has no stored {provider} refresh token — run \
                     `oauth start {provider}` first"
                )));
            }
            let tokens = registry
                .oauth
                .refresh_token(&provider, &stored_refresh)
                .await?;
            // Persist the rotated credentials — providers that rotate
            // refresh tokens invalidate the old one on use.
            let now = chrono::Utc::now().timestamp();
            let expires_at = tokens
                .expires_in
                .map(|s| now.saturating_add(i64::try_from(s).unwrap_or(i64::MAX)))
                .unwrap_or(0);
            let account = match provider.as_str() {
                "twitch" => &mut p.settings.oauth.twitch,
                "youtube" => &mut p.settings.oauth.youtube,
                "kick" => &mut p.settings.oauth.kick,
                "trovo" => &mut p.settings.oauth.trovo,
                _ => &mut p.settings.oauth.facebook,
            };
            account.access_token = tokens.access_token.clone();
            if let Some(rt) = tokens.refresh_token.clone() {
                account.refresh_token = rt;
            }
            account.expires_at = expires_at;
            registry
                .profiles
                .save_with_key_encryption(&p, password.as_deref())
                .await?;
            out.emit(&serde_json::json!({
                "provider": provider,
                "refreshed": true,
                "expiresAt": expires_at,
            }))?;
            Ok(())
        }
        OAuthCmd::Forget {
            provider,
            token_from,
        } => {
            let token = crate::secret_input::read_secret(token_from, "OAuth token to revoke")?;
            // Revoke the supplied token (best effort) then report success.
            if let Err(e) = registry.oauth.revoke_token(&provider, &token).await {
                log::warn!("revoke {provider} token failed: {e}");
            }
            out.emit(&serde_json::json!({ "provider": provider, "forgotten": true }))?;
            Ok(())
        }
        OAuthCmd::Account {
            provider,
            profile,
            password_from,
        } => {
            let password =
                crate::secret_input::read_optional_secret(password_from, "Profile password")?;
            let profile_obj = registry
                .profiles
                .load_with_key_decryption(&profile, password.as_deref())
                .await?;
            let account = match provider.as_str() {
                "twitch" => &profile_obj.settings.oauth.twitch,
                "youtube" => &profile_obj.settings.oauth.youtube,
                "kick" => &profile_obj.settings.oauth.kick,
                "facebook" => &profile_obj.settings.oauth.facebook,
                "trovo" => &profile_obj.settings.oauth.trovo,
                other => {
                    return Err(CliError::Argument(format!(
                        "unknown oauth provider: {other} (expected twitch|youtube|kick|facebook|trovo)"
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
                // Per-provider setup summaries (same core source the
                // HTTP config endpoint serves). Secret VALUES are
                // deliberately omitted — `configured` already conveys
                // their presence.
                let summaries = registry
                    .oauth
                    .provider_summaries(&std::collections::HashMap::new())
                    .await;
                out.emit(&serde_json::json!({ "providers": summaries }))?;
                Ok(())
            }
            ConfigCmd::SetProvider {
                provider,
                client_id,
                client_secret_from,
            } => {
                let secret =
                    crate::secret_input::read_optional_secret(client_secret_from, "client secret")?;
                registry
                    .oauth
                    .set_provider_credentials(&provider, client_id, secret.map(|s| s.to_string()))
                    .await?;
                let summaries = registry
                    .oauth
                    .provider_summaries(&std::collections::HashMap::new())
                    .await;
                out.emit(&serde_json::json!({ "providers": summaries }))?;
                Ok(())
            }
            ConfigCmd::Set(args) => {
                let ConfigSetArgs {
                    json,
                    twitch_client_id,
                    twitch_client_secret,
                    youtube_client_id,
                    youtube_client_secret,
                    kick_client_id,
                    kick_client_secret,
                    facebook_client_id,
                    facebook_client_secret,
                    trovo_client_id,
                    trovo_client_secret,
                } = *args;
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
                        trovo_client_id,
                        trovo_client_secret,
                    }
                };
                registry.oauth.update_config(config).await?;
                out.emit(&serde_json::json!({ "updated": true }))?;
                Ok(())
            }
        },
    }
}
