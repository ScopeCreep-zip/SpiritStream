//! `spiritstream-cli discord …` — Discord webhook integration.

use clap::Subcommand;
use spiritstream_core::ServiceRegistry;

use crate::error::CliError;
use crate::output::Output;

#[derive(Debug, Subcommand)]
pub enum DiscordCmd {
    /// Send a test message to a Discord webhook URL.
    TestWebhook { url: String },
    /// Send the go-live notification using `--profile <name>`'s saved
    /// Discord settings. Loads the named profile (decrypting with
    /// `--password` if encrypted), reads `settings.discord`, and calls
    /// the same `send_go_live_notification` the HTTP route invokes.
    Send {
        #[arg(long)]
        profile: String,
        #[arg(long)]
        password: Option<String>,
    },
    /// Reset the cooldown timer so the next send fires immediately.
    ResetCooldown,
}

pub async fn run(
    cmd: DiscordCmd,
    registry: &ServiceRegistry,
    out: &mut Output,
) -> Result<(), CliError> {
    match cmd {
        DiscordCmd::TestWebhook { url } => {
            let result = registry.discord.test_webhook(&url).await;
            out.emit(&result)?;
            Ok(())
        }
        DiscordCmd::Send { profile, password } => {
            let profile_obj = registry
                .profiles
                .load_with_key_decryption(&profile, password.as_deref())
                .await?;
            let discord = &profile_obj.settings.discord;
            if !discord.webhook_enabled {
                out.emit(&serde_json::json!({
                    "success": false,
                    "message": "Discord webhook is not enabled",
                    "skippedCooldown": false,
                }))?;
                return Ok(());
            }
            let image_path = if discord.image_path.is_empty() {
                None
            } else {
                Some(discord.image_path.as_str())
            };
            let result = registry
                .discord
                .send_go_live_notification(
                    &discord.webhook_url,
                    &discord.go_live_message,
                    image_path,
                    discord.cooldown_enabled,
                    discord.cooldown_seconds,
                )
                .await;
            out.emit(&result)?;
            Ok(())
        }
        DiscordCmd::ResetCooldown => {
            registry.discord.reset_cooldown().await;
            out.emit(&serde_json::json!({ "reset": true }))?;
            Ok(())
        }
    }
}
