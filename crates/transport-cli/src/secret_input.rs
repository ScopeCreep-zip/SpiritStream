//! Secret entry for CLI commands — never via argv.
//!
//! Command-line arguments are visible to every same-user process
//! (`ps`, shell history, crash dumps) — unacceptable for the shared- /
//! observed-machine users SpiritStream serves. The CLI therefore has
//! exactly three ways to obtain a secret, mirroring the docker / gh
//! model:
//!
//! 1. **By reference** — commands that operate on secrets core already
//!    stores (profile-saved OAuth tokens, etc.) take names, not values.
//! 2. **`--*-from prompt`** — interactive TTY entry via `rpassword`
//!    (no echo).
//! 3. **`--*-from stdin`** — one secret per invocation piped on stdin
//!    (first line), for scripts: `printf '%s' "$KEY" | spiritstream-cli …`.
//!
//! There are deliberately NO plaintext secret flags left. A non-TTY
//! invocation that asks for `prompt` gets an instructive error instead
//! of hanging.

use std::io::{BufRead, IsTerminal};

use zeroize::Zeroizing;

use crate::error::CliError;

/// Where a secret comes from. Used as `--password-from`,
/// `--stream-key-from`, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum SecretSource {
    /// Read the first line from stdin (for scripts / pipes).
    Stdin,
    /// Prompt interactively on the terminal (input is not echoed).
    Prompt,
}

/// Read one secret from the given source.
pub fn read_secret(source: SecretSource, label: &str) -> Result<Zeroizing<String>, CliError> {
    match source {
        SecretSource::Stdin => {
            let mut line = String::new();
            std::io::stdin()
                .lock()
                .read_line(&mut line)
                .map_err(|e| CliError::Io(format!("reading secret from stdin: {e}")))?;
            let trimmed = line.trim_end_matches(['\r', '\n']).to_string();
            if trimmed.is_empty() {
                return Err(CliError::Argument(format!(
                    "{label}: stdin was empty — pipe the secret, e.g. \
                     printf '%s' \"$SECRET\" | spiritstream-cli …"
                )));
            }
            Ok(Zeroizing::new(trimmed))
        }
        SecretSource::Prompt => {
            if !std::io::stdin().is_terminal() {
                return Err(CliError::Argument(format!(
                    "{label}: --…-from prompt needs an interactive terminal; \
                     use --…-from stdin and pipe the secret in scripts"
                )));
            }
            let value = rpassword::prompt_password(format!("{label}: "))
                .map_err(|e| CliError::Io(format!("reading secret from terminal: {e}")))?;
            if value.is_empty() {
                return Err(CliError::Argument(format!("{label}: empty input")));
            }
            Ok(Zeroizing::new(value))
        }
    }
}

/// Optional-secret convenience: `None` source ⇒ `None` secret.
pub fn read_optional_secret(
    source: Option<SecretSource>,
    label: &str,
) -> Result<Option<String>, CliError> {
    match source {
        None => Ok(None),
        Some(source) => Ok(Some(read_secret(source, label)?.to_string())),
    }
}

/// Read `name:password` pairs, one per line, from stdin — used by
/// `data rotate-machine-key --passwords-stdin` to supply unlock
/// passwords for every encrypted profile in one pipe.
pub fn read_password_pairs_from_stdin(
) -> Result<std::collections::HashMap<String, String>, CliError> {
    let mut pairs = std::collections::HashMap::new();
    for line in std::io::stdin().lock().lines() {
        let line = line.map_err(|e| CliError::Io(format!("reading passwords from stdin: {e}")))?;
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let (name, password) = line.split_once(':').ok_or_else(|| {
            CliError::Argument(
                "expected one name:password pair per line on stdin".to_string(),
            )
        })?;
        pairs.insert(name.to_string(), password.to_string());
    }
    Ok(pairs)
}
