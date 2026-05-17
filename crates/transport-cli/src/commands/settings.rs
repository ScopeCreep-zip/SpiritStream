//! `spiritstream-cli settings …` — global app-wide settings.

use clap::Subcommand;
use serde_json::Value;
use spiritstream_core::ServiceRegistry;

use crate::error::CliError;
use crate::output::Output;

#[derive(Debug, Subcommand)]
pub enum SettingsCmd {
    /// Print resolved global settings as JSON. When `<key>` is provided,
    /// print just that top-level field. Unknown keys exit with an
    /// `argument` error (EX_USAGE).
    Get {
        /// Optional top-level settings key (e.g. `logRetentionDays`).
        key: Option<String>,
    },
    /// Apply field changes provided as one or more `--set key=value` flags.
    /// Values are parsed as JSON when possible (numbers, booleans, strings),
    /// falling back to a raw string when JSON parsing fails. Out-of-range
    /// values surface as `validation_failed` and exit code 7.
    Set {
        #[arg(long = "set", value_name = "KEY=VALUE", required = true)]
        overrides: Vec<String>,
    },
    /// Print the absolute path of the profiles directory.
    ProfilesPath,
}

pub async fn run(
    cmd: SettingsCmd,
    registry: &ServiceRegistry,
    out: &mut Output,
) -> Result<(), CliError> {
    match cmd {
        SettingsCmd::Get { key: None } => {
            let settings = registry.settings.load()?;
            out.emit(&settings)?;
            Ok(())
        }
        SettingsCmd::Get { key: Some(key) } => {
            let settings = registry.settings.load()?;
            let json = serde_json::to_value(&settings)
                .map_err(|e| CliError::Serialization(e.to_string()))?;
            match json.get(&key) {
                Some(value) => {
                    out.emit(value)?;
                    Ok(())
                }
                None => Err(CliError::Argument(format!("unknown settings key: {key}"))),
            }
        }
        SettingsCmd::Set { overrides } => {
            let settings = registry.settings.load()?;
            let mut json = serde_json::to_value(&settings)
                .map_err(|e| CliError::Serialization(e.to_string()))?;

            for raw in &overrides {
                let (key, value_str) = raw
                    .split_once('=')
                    .ok_or_else(|| CliError::Argument(format!("expected key=value, got: {raw}")))?;
                let key = key.trim();
                let value_str = value_str.trim();

                let parsed: Value = serde_json::from_str(value_str)
                    .unwrap_or_else(|_| Value::String(value_str.to_owned()));

                let obj = json.as_object_mut().ok_or_else(|| {
                    CliError::Serialization("settings is not a JSON object".into())
                })?;

                if !obj.contains_key(key) {
                    return Err(CliError::Argument(format!("unknown settings key: {key}")));
                }
                obj.insert(key.to_owned(), parsed);
            }

            let updated: spiritstream_core::models::Settings = serde_json::from_value(json)
                .map_err(|e| CliError::Serialization(format!("invalid settings shape: {e}")))?;

            registry.settings.save(&updated)?;
            out.emit(&updated)?;
            Ok(())
        }
        SettingsCmd::ProfilesPath => {
            let path = registry.settings.get_profiles_path();
            out.emit(&serde_json::json!({ "path": path.to_string_lossy() }))?;
            Ok(())
        }
    }
}
