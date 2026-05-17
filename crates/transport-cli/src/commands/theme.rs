//! `spiritstream-cli theme …` — installed theme catalog.

use clap::Subcommand;
use spiritstream_core::ServiceRegistry;

use crate::error::CliError;
use crate::output::Output;

#[derive(Debug, Subcommand)]
pub enum ThemeCmd {
    /// List every theme the catalog can serve.
    List,
    /// Resolve a theme's token dictionary by ID.
    Tokens { id: String },
    /// Install a `.spirittheme.json` file into the user theme catalog.
    Install { path: std::path::PathBuf },
    /// Re-sync the project theme directory into the catalog.
    Refresh,
}

pub async fn run(
    cmd: ThemeCmd,
    registry: &ServiceRegistry,
    out: &mut Output,
) -> Result<(), CliError> {
    match cmd {
        ThemeCmd::List => {
            let themes = registry.themes.list_themes();
            out.emit(&themes)?;
            Ok(())
        }
        ThemeCmd::Tokens { id } => {
            let tokens = registry.themes.get_theme_tokens(&id)?;
            out.emit(&tokens)?;
            Ok(())
        }
        ThemeCmd::Install { path } => {
            let summary = registry.themes.install_theme(&path)?;
            out.emit(&summary)?;
            Ok(())
        }
        ThemeCmd::Refresh => {
            registry.themes.sync_project_themes();
            let themes = registry.themes.list_themes();
            out.emit(&themes)?;
            Ok(())
        }
    }
}
