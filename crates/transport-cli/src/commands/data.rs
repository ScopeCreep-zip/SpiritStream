//! `spiritstream-cli data …` — data lifecycle commands.

use clap::Subcommand;
use serde::Serialize;
use spiritstream_core::ServiceRegistry;

use crate::error::CliError;
use crate::output::Output;

#[derive(Debug, Subcommand)]
pub enum DataCmd {
    /// Export profiles, settings, and logs to a directory at `<path>`.
    Export {
        /// Destination directory. Must live inside the user's home or data dir.
        path: std::path::PathBuf,
    },
    /// **Destructive.** Delete every profile and reset settings to defaults.
    Clear {
        /// Confirm the wipe. Refused without `--yes`.
        #[arg(long)]
        yes: bool,
    },
    /// Rotate the machine encryption key. Re-encrypts every encrypted profile.
    RotateMachineKey {
        /// Confirm the rotation. Refused without `--yes` — recovery requires
        /// the old key file if anything goes wrong mid-rotation.
        #[arg(long)]
        yes: bool,
        /// Password for an encrypted (`.mgs`) profile, repeatable as
        /// `--password NAME=PASSWORD`. Required once per encrypted profile;
        /// rotation refuses to start if any are missing.
        #[arg(long, value_name = "NAME=PASSWORD")]
        password: Vec<String>,
    },
}

#[derive(Serialize)]
struct ExportResponse {
    path: std::path::PathBuf,
    exported: bool,
}

#[derive(Serialize)]
struct ClearResponse {
    cleared: bool,
}

pub async fn run(
    cmd: DataCmd,
    registry: &ServiceRegistry,
    out: &mut Output,
) -> Result<(), CliError> {
    match cmd {
        DataCmd::Export { path } => {
            registry.settings.export_data(&path)?;
            out.emit(&ExportResponse {
                path,
                exported: true,
            })?;
            Ok(())
        }
        DataCmd::Clear { yes } => {
            if !yes {
                return Err(CliError::Argument(
                    "data clear is destructive — pass --yes to confirm".into(),
                ));
            }
            registry.settings.clear_data()?;
            out.emit(&ClearResponse { cleared: true })?;
            Ok(())
        }
        DataCmd::RotateMachineKey { yes, password } => {
            if !yes {
                return Err(CliError::Argument(
                    "rotate-machine-key changes encryption — pass --yes to confirm".into(),
                ));
            }
            let mut passwords: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();
            for entry in password {
                let (name, pw) = entry.split_once('=').ok_or_else(|| {
                    CliError::Argument(
                        "--password expects NAME=PASSWORD per encrypted profile".into(),
                    )
                })?;
                passwords.insert(name.to_string(), pw.to_string());
            }
            // Shared core orchestration: refuses while streams are live
            // and records `MachineKeyRotated` in the HMAC chain —
            // identical rules for HTTP and CLI by construction.
            let report = registry.rotate_machine_key_checked(&passwords)?;
            out.emit(&report)?;
            Ok(())
        }
    }
}
