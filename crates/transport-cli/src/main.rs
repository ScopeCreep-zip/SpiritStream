//! `spiritstream-cli` — first-class headless client for SpiritStream.
//!
//! Every operation the React UI can perform is reachable from this binary.
//! Commands dispatch **in-process** against `spiritstream-core::ServiceRegistry`
//! — there is no HTTP round-trip. That property is what makes the test
//! substrate under `tests/integration/` reproducible: every test is a shell
//! invocation against a `--data-dir <tmpdir>` install with deterministic JSON
//! output and stable exit codes.
//!
//! Run-of-the-mill commands print one JSON document on stdout. Use `--pretty`
//! for indented output (human review) and `--quiet` to suppress informational
//! log lines on stderr.

mod commands;
mod error;
mod output;
mod registry;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::error::CliError;
use crate::output::Output;

#[derive(Debug, Parser)]
#[command(
    name = "spiritstream-cli",
    version = env!("CARGO_PKG_VERSION"),
    about = "Headless client for SpiritStream — in-process dispatch into spiritstream-core.",
    long_about = None,
)]
struct Cli {
    /// Override the data directory. Defaults to the platform user data dir
    /// (same lookup as the desktop sidecar), or `$SPIRITSTREAM_DATA_DIR`
    /// if set.
    #[arg(long, global = true, env = "SPIRITSTREAM_DATA_DIR")]
    data_dir: Option<std::path::PathBuf>,

    /// Override the themes directory (mostly useful for tests).
    #[arg(long, global = true, env = "SPIRITSTREAM_THEMES_DIR")]
    themes_dir: Option<std::path::PathBuf>,

    /// Pretty-print JSON output for human reading.
    #[arg(long, global = true)]
    pretty: bool,

    /// Suppress non-error log lines on stderr.
    #[arg(long, global = true)]
    quiet: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Manage stream profiles.
    Profile {
        #[command(subcommand)]
        command: commands::profile::ProfileCmd,
    },
    /// Observe and control RTMP streams.
    Stream {
        #[command(subcommand)]
        command: commands::stream::StreamCmd,
    },
    /// Chat platform connections and messaging.
    Chat {
        #[command(subcommand)]
        command: commands::chat::ChatCmd,
    },
    /// Inspect system capabilities (encoders, FFmpeg).
    System {
        #[command(subcommand)]
        command: commands::system::SystemCmd,
    },
    /// Inspect or update global settings.
    Settings {
        #[command(subcommand)]
        command: commands::settings::SettingsCmd,
    },
    /// List available themes.
    Theme {
        #[command(subcommand)]
        command: commands::theme::ThemeCmd,
    },
    /// Export / import / clear local data.
    Data {
        #[command(subcommand)]
        command: commands::data::DataCmd,
    },
    /// OAuth 2.0 flows for chat platforms.
    Oauth {
        #[command(subcommand)]
        command: commands::oauth::OAuthCmd,
    },
    /// OBS Studio WebSocket integration.
    Obs {
        #[command(subcommand)]
        command: commands::obs::ObsCmd,
    },
    /// Discord webhook integration.
    Discord {
        #[command(subcommand)]
        command: commands::discord::DiscordCmd,
    },
    /// File-browser primitives (browse directories, read home dir).
    Files {
        #[command(subcommand)]
        command: commands::files::FilesCmd,
    },
    /// Subscribe to real-time service events.
    Events {
        #[command(subcommand)]
        command: commands::events::EventsCmd,
    },
    /// Safety features for vulnerable users.
    Safety {
        #[command(subcommand)]
        command: commands::safety::SafetyCmd,
    },
    /// Read the local audit log.
    Audit {
        #[command(subcommand)]
        command: commands::audit::AuditCmd,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    // Logging goes to stderr; --quiet suppresses anything below error.
    let level = if cli.quiet { "error" } else { "info" };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(level))
        .target(env_logger::Target::Stderr)
        .init();

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("failed to start tokio runtime: {e}");
            return ExitCode::from(70); // EX_SOFTWARE
        }
    };

    let mut out = Output::new(cli.pretty);
    let result = runtime.block_on(run(cli, &mut out));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            out.emit_error(&err);
            ExitCode::from(err.exit_code())
        }
    }
}

async fn run(cli: Cli, out: &mut Output) -> Result<(), CliError> {
    let registry = registry::build(cli.data_dir, cli.themes_dir)?;
    match cli.command {
        Command::Profile { command } => commands::profile::run(command, &registry, out).await,
        Command::Stream { command } => commands::stream::run(command, &registry, out).await,
        Command::Chat { command } => commands::chat::run(command, &registry, out).await,
        Command::System { command } => commands::system::run(command, &registry, out).await,
        Command::Settings { command } => commands::settings::run(command, &registry, out).await,
        Command::Theme { command } => commands::theme::run(command, &registry, out).await,
        Command::Data { command } => commands::data::run(command, &registry, out).await,
        Command::Oauth { command } => commands::oauth::run(command, &registry, out).await,
        Command::Obs { command } => commands::obs::run(command, &registry, out).await,
        Command::Discord { command } => commands::discord::run(command, &registry, out).await,
        Command::Files { command } => commands::files::run(command, &registry, out).await,
        Command::Events { command } => commands::events::run(command, &registry, out).await,
        Command::Safety { command } => commands::safety::run(command, &registry, out).await,
        Command::Audit { command } => commands::audit::run(command, &registry, out).await,
    }
}
