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
        /// Read `name:password` unlock pairs for encrypted (`.mgs`)
        /// profiles from stdin, one per line. Without this flag, an
        /// interactive terminal prompts per encrypted profile (no echo);
        /// passwords never ride argv. Rotation refuses to start if any
        /// encrypted profile's password is missing.
        #[arg(long)]
        passwords_stdin: bool,
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
        DataCmd::RotateMachineKey {
            yes,
            passwords_stdin,
        } => {
            if !yes {
                return Err(CliError::Argument(
                    "rotate-machine-key changes encryption — pass --yes to confirm".into(),
                ));
            }
            let passwords = if passwords_stdin {
                crate::secret_input::read_password_pairs_from_stdin()?
            } else {
                // Interactive path: prompt (no echo) for each encrypted
                // profile on disk. Non-TTY without --passwords-stdin and
                // with encrypted profiles present fails loudly inside
                // read_secret.
                let mut pairs = std::collections::HashMap::new();
                let profiles_dir = registry.data_dir.join("profiles");
                if let Ok(entries) = std::fs::read_dir(&profiles_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().and_then(|e| e.to_str()) == Some("mgs") {
                            let name = path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or_default()
                                .to_string();
                            let pw = crate::secret_input::read_secret(
                                crate::secret_input::SecretSource::Prompt,
                                &format!("Password for encrypted profile '{name}'"),
                            )?;
                            pairs.insert(name, pw.to_string());
                        }
                    }
                }
                pairs
            };
            // Shared core orchestration: refuses while streams are live
            // and records `MachineKeyRotated` in the HMAC chain —
            // identical rules for HTTP and CLI by construction.
            let report = registry.rotate_machine_key_checked(&passwords)?;
            out.emit(&report)?;
            Ok(())
        }
    }
}
