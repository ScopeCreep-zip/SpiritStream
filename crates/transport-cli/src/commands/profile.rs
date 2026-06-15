//! `spiritstream-cli profile …` — manage stream profiles.

use clap::Subcommand;
use serde::Serialize;
use spiritstream_core::models::Profile;
use spiritstream_core::ServiceRegistry;

use crate::error::CliError;
use crate::output::Output;

#[derive(Debug, Subcommand)]
pub enum ProfileCmd {
    /// List every profile by name in user-defined order.
    List,
    /// Report whether a profile exists locally.
    Exists {
        /// Profile name to query.
        name: String,
    },
    /// Print the resolved profile as JSON. `--password` is required for
    /// encrypted profiles and silently ignored for plaintext ones.
    Show {
        name: String,
        #[arg(long = "password-from", value_enum)]
        password_from: Option<crate::secret_input::SecretSource>,
    },
    /// Persist a profile from a JSON document on disk. `--password`
    /// re-encrypts the profile under that password (omit to leave plaintext).
    Save {
        /// Path to a JSON file containing the profile body.
        file: std::path::PathBuf,
        #[arg(long = "password-from", value_enum)]
        password_from: Option<crate::secret_input::SecretSource>,
    },
    /// Edit a profile's settings by dotted key path: load → mutate → save.
    /// One or more `--set key=value` target `settings.*` (e.g. `obs.host`,
    /// `obs.autoConnect`, `obs.direction`, `discord.webhookEnabled`,
    /// `chat.twitchChannel`). Values parse as JSON when possible, else a raw
    /// string; unknown keys exit with an `argument` error. Secrets
    /// (`obs.password`, …) are encrypted at the save boundary — the CLI never
    /// encrypts them itself. This is the headless equivalent of the GUI's
    /// settings forms. Persists only; a running server re-derives its runtime
    /// on its own save path.
    Set {
        name: String,
        #[arg(long = "set", value_name = "KEY=VALUE", required = true)]
        overrides: Vec<String>,
        #[arg(long = "password-from", value_enum)]
        password_from: Option<crate::secret_input::SecretSource>,
    },
    /// Delete a profile.
    Delete { name: String },
    /// Report whether the named profile is stored encrypted on disk.
    IsEncrypted { name: String },
    /// Print the resolved summary list — parsed numeric bitrate/resolution
    /// and unique services, sourced from `Profile::to_summary()`.
    Summaries,
    /// Validate an RTMP input shape (bind address + port + application) for
    /// port-conflict against other profiles. Reads input JSON from `<file>`.
    ValidateInput {
        profile_id: String,
        file: std::path::PathBuf,
    },
    /// Replace the saved profile-order map (priority order shown in the UI).
    Reorder { ordered_names: Vec<String> },
    /// Print the saved profile-order map.
    Order,
    /// Backfill order indexes for any profile missing one. Idempotent —
    /// no-op when every profile already appears in the map. Mirrors
    /// `POST /api/v1/profiles/order/ensure`.
    EnsureOrder,
    /// Activate a profile — runs `ProfileService::activate()` so chat settings
    /// propagate, OBS reconfigures and (when the profile asks for it) auto-
    /// connects. Prints the resolved profile + the `ProfileActivatedEvent`
    /// payload the HTTP transport would emit on `/api/v1/events`.
    Activate {
        name: String,
        #[arg(long = "password-from", value_enum)]
        password_from: Option<crate::secret_input::SecretSource>,
    },
    /// Sign out of the active profile — runs `ProfileActivationService::
    /// deactivate()`. Clears the persisted active profile, drops the
    /// anonymizer salt from memory, disconnects chat + OBS, and emits
    /// `profile_deactivated`. Mirrors `POST /api/v1/profiles/deactivate`.
    /// Prints the name of the profile that was active (or `null`).
    Deactivate,
    /// Atomically remove encryption from a profile: load with `--password`,
    /// re-save unencrypted. Mirrors `POST /api/v1/profiles/{name}/decrypt`.
    Decrypt {
        name: String,
        #[arg(long = "password-from", value_enum)]
        password_from: crate::secret_input::SecretSource,
    },
    /// Verify a profile password against the encrypted blob. Mirrors
    /// `POST /api/v1/profiles/{name}/unlock`. The HTTP transport's session
    /// unlock set lives on `AppState`; in the CLI each invocation is its own
    /// session, so this command only validates the password.
    Unlock {
        name: String,
        #[arg(long = "password-from", value_enum)]
        password_from: crate::secret_input::SecretSource,
    },
    /// Mirrors `POST /api/v1/profiles/{name}/lock`. CLI is single-shot so
    /// this is a confirmation echo — no cross-invocation session state.
    Lock { name: String },
    /// Mirrors `GET /api/v1/profiles/locked`. CLI is single-shot so the
    /// unlocked set is always empty.
    LockedList,
}

#[derive(Serialize)]
struct ListResponse<'a> {
    names: &'a [String],
}

#[derive(Serialize)]
struct ExistsResponse {
    name: String,
    exists: bool,
}

#[derive(Serialize)]
struct DeleteResponse {
    name: String,
    deleted: bool,
}

#[derive(Serialize)]
struct SaveResponse {
    name: String,
    saved: bool,
}

#[derive(Serialize)]
struct IsEncryptedResponse {
    name: String,
    encrypted: bool,
}

pub async fn run(
    cmd: ProfileCmd,
    registry: &ServiceRegistry,
    out: &mut Output,
) -> Result<(), CliError> {
    match cmd {
        ProfileCmd::List => {
            let names = registry.profiles.get_all_names().await?;
            out.emit(&ListResponse { names: &names })?;
            Ok(())
        }
        ProfileCmd::Exists { name } => {
            let names = registry.profiles.get_all_names().await?;
            let exists = names.iter().any(|n| n == &name);
            out.emit(&ExistsResponse { name, exists })?;
            Ok(())
        }
        ProfileCmd::Show {
            name,
            password_from,
        } => {
            let password =
                crate::secret_input::read_optional_secret(password_from, "Profile password")?;
            let profile = registry
                .profiles
                .load_with_key_decryption(&name, password.as_deref())
                .await?;
            out.emit(&profile)?;
            Ok(())
        }
        ProfileCmd::Save {
            file,
            password_from,
        } => {
            let password =
                crate::secret_input::read_optional_secret(password_from, "Profile password")?;
            let body = std::fs::read_to_string(&file)
                .map_err(|e| CliError::Io(format!("read {}: {}", file.display(), e)))?;
            let profile: Profile = serde_json::from_str(&body)
                .map_err(|e| CliError::Serialization(format!("parse {}: {}", file.display(), e)))?;
            let name = profile.name.clone();
            registry
                .profiles
                .save_with_key_encryption(&profile, password.as_deref())
                .await?;
            out.emit(&SaveResponse { name, saved: true })?;
            Ok(())
        }
        ProfileCmd::Set {
            name,
            overrides,
            password_from,
        } => {
            let password =
                crate::secret_input::read_optional_secret(password_from, "Profile password")?;
            let mut profile = registry
                .profiles
                .load_with_key_decryption(&name, password.as_deref())
                .await?;

            // Mutate the settings subtree as JSON, then re-type it back through
            // serde so the typed `ProfileSettings` shape/bounds/enums validate
            // (a bad `obs.direction` fails here as `serialization`/EX_DATAERR).
            let mut settings_json = serde_json::to_value(&profile.settings)
                .map_err(|e| CliError::Serialization(e.to_string()))?;
            for raw in &overrides {
                let (key, value) = crate::commands::json_path::parse_override(raw)?;
                crate::commands::json_path::set_dotted(&mut settings_json, key, value)?;
            }
            profile.settings = serde_json::from_value(settings_json)
                .map_err(|e| CliError::Serialization(format!("invalid settings shape: {e}")))?;

            // Save encrypts secrets at the boundary (`visit_secret_fields`).
            registry
                .profiles
                .save_with_key_encryption(&profile, password.as_deref())
                .await?;
            out.emit(&profile.settings)?;
            Ok(())
        }
        ProfileCmd::Delete { name } => {
            registry.profiles.delete(&name).await?;
            out.emit(&DeleteResponse {
                name,
                deleted: true,
            })?;
            Ok(())
        }
        ProfileCmd::IsEncrypted { name } => {
            let encrypted = registry.profiles.is_encrypted(&name);
            out.emit(&IsEncryptedResponse { name, encrypted })?;
            Ok(())
        }
        ProfileCmd::Summaries => {
            let summaries = registry.profiles.get_all_summaries().await?;
            out.emit(&summaries)?;
            Ok(())
        }
        ProfileCmd::ValidateInput { profile_id, file } => {
            let body = std::fs::read_to_string(&file)
                .map_err(|e| CliError::Io(format!("read {}: {}", file.display(), e)))?;
            let input: spiritstream_core::models::RtmpInput = serde_json::from_str(&body)
                .map_err(|e| CliError::Serialization(format!("parse {}: {}", file.display(), e)))?;
            registry
                .profiles
                .validate_input_conflict(&profile_id, &input)
                .await?;
            out.emit(&serde_json::json!({ "valid": true }))?;
            Ok(())
        }
        ProfileCmd::Reorder { ordered_names } => {
            // Mirror the HTTP `PATCH /api/v1/profiles/order` behaviour: every
            // submitted name must already exist; missing ones are rejected
            // before any writes.
            let existing = registry.profiles.get_all_names().await?;
            let mut map = registry.profiles.read_order_index_map()?;
            let mut idx: i32 = 0;
            for name in &ordered_names {
                if !existing.contains(name) {
                    return Err(spiritstream_core::CoreError::ProfileNotFound {
                        name: name.clone(),
                    }
                    .into());
                }
                idx += 10;
                map.insert(name.clone(), idx);
            }
            registry.profiles.write_order_index_map(&map)?;
            out.emit(&serde_json::json!({ "reordered": ordered_names }))?;
            Ok(())
        }
        ProfileCmd::Order => {
            let map = registry.profiles.read_order_index_map()?;
            out.emit(&map)?;
            Ok(())
        }
        ProfileCmd::EnsureOrder => {
            let map = registry.profiles.ensure_order_indexes().await?;
            out.emit(&map)?;
            Ok(())
        }
        ProfileCmd::Activate {
            name,
            password_from,
        } => {
            let password =
                crate::secret_input::read_optional_secret(password_from, "Profile password")?;
            // ProfileActivationService composes load + OAuth refresh +
            // chat/OBS propagation + bus emission in a single call.
            // CLI and HTTP share the same orchestrator — wire shape and
            // log volume are identical across transports.
            let outcome = registry
                .profile_activation
                .activate(&name, password.as_deref())
                .await?;
            out.emit(&serde_json::json!({
                "profile": outcome.profile,
                "event": outcome.event,
                "oauthRefreshFailed": outcome.oauth_refresh_failed,
            }))?;
            Ok(())
        }
        ProfileCmd::Deactivate => {
            // Stateless CLI: the core orchestrator clears persisted +
            // in-memory per-profile state. There's no long-lived session
            // snapshot to clear (that's the HTTP transport's concern).
            let outcome = registry.profile_activation.deactivate().await?;
            out.emit(&serde_json::json!({ "deactivated": outcome.deactivated }))?;
            Ok(())
        }
        ProfileCmd::Decrypt {
            name,
            password_from,
        } => {
            let password = crate::secret_input::read_secret(password_from, "Profile password")?;
            let profile = registry
                .profiles
                .load_with_key_decryption(&name, Some(&password))
                .await?;
            registry
                .profiles
                .save_with_key_encryption(&profile, None)
                .await?;
            out.emit(&serde_json::json!({ "name": name, "decrypted": true }))?;
            Ok(())
        }
        ProfileCmd::Unlock {
            name,
            password_from,
        } => {
            let password = crate::secret_input::read_secret(password_from, "Profile password")?;
            // Validates the password by attempting decryption; the HTTP
            // transport additionally tracks an in-memory unlock set on
            // AppState, but the CLI is a one-shot process so the
            // verification IS the unlock for this invocation.
            registry
                .profiles
                .load_with_key_decryption(&name, Some(&password))
                .await?;
            out.emit(&serde_json::json!({ "name": name, "unlocked": true }))?;
            Ok(())
        }
        ProfileCmd::Lock { name } => {
            out.emit(&serde_json::json!({ "name": name, "locked": true }))?;
            Ok(())
        }
        ProfileCmd::LockedList => {
            out.emit(&serde_json::json!({ "unlocked": Vec::<String>::new() }))?;
            Ok(())
        }
    }
}
