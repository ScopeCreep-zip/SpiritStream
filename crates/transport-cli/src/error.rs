//! CLI-level error type that maps `CoreError` (and other failure modes) to
//! stable exit codes consumers can branch on.

use spiritstream_core::CoreError;

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("{0}")]
    Core(#[from] CoreError),

    #[error("io: {0}")]
    Io(String),

    #[error("serialization: {0}")]
    Serialization(String),

    #[error("invalid argument: {0}")]
    Argument(String),

    /// Service-level "not ready" / "transient failure" — distinct from
    /// `Argument` so retry-loop scripts (`until cli system ready; do
    /// sleep 5; done`) can tell "the service hasn't come up yet" from
    /// "I called you with bad flags." Maps to exit 69 (EX_UNAVAILABLE),
    /// mirroring `CoreError::NetworkError`.
    #[error("service unavailable: {0}")]
    Unavailable(String),
}

impl CliError {
    /// POSIX-style exit code. Inspired by `sysexits.h` where applicable, with
    /// SpiritStream-specific codes for domain failures.
    pub fn exit_code(&self) -> u8 {
        match self {
            CliError::Core(err) => match err {
                CoreError::ProfileNotFound { .. } => 4,
                CoreError::PasswordRequired { .. } => 5,
                CoreError::PasswordIncorrect => 6,
                CoreError::PasswordTooShort { .. } => 7, // EX_USAGE — caller supplied invalid input
                CoreError::InvalidStreamConfig { .. } => 7,
                CoreError::ValidationFailed { .. } => 7,
                CoreError::EncoderUnavailable { .. } => 8,
                CoreError::PortConflict { .. } => 9,
                CoreError::ProfileAlreadyExists { .. } => 9,
                CoreError::FfmpegNotFound => 10,
                CoreError::PathOutsideAllowedRoot { .. } => 13,
                CoreError::NotFound { .. } => 4,
                CoreError::RateLimited { .. } => 14,
                CoreError::Unauthorized => 15,
                CoreError::NoActiveProfile => 16,
                CoreError::ChatPlatformNotConnected { .. } => 17,
                CoreError::ChatSendingDisabled { .. } => 18,
                CoreError::ChatMessageLengthExceeded { .. } => 7,
                CoreError::ChatBlockedByPii { .. } => 19,
                CoreError::AnonymousSaltInvalid => 20,
                CoreError::NotImplemented { .. } => 78, // EX_CONFIG
                CoreError::NetworkError { .. } => 69,   // EX_UNAVAILABLE
                CoreError::Internal { .. } => 70,       // EX_SOFTWARE
            },
            CliError::Argument(_) => 64,      // EX_USAGE
            CliError::Unavailable(_) => 69,   // EX_UNAVAILABLE
            CliError::Io(_) => 74,            // EX_IOERR
            CliError::Serialization(_) => 65, // EX_DATAERR
        }
    }

    /// Stable error kind string for JSON output. Consumers can branch on this
    /// without needing to parse the human-readable message. For `Core`
    /// failures it delegates to `CoreError::kind()` so the CLI and HTTP
    /// transports surface identical kinds for the same underlying error.
    pub fn kind(&self) -> &'static str {
        match self {
            CliError::Core(err) => err.kind(),
            CliError::Argument(_) => "argument",
            CliError::Unavailable(_) => "unavailable",
            CliError::Io(_) => "io",
            CliError::Serialization(_) => "serialization",
        }
    }
}

impl From<std::io::Error> for CliError {
    fn from(err: std::io::Error) -> Self {
        CliError::Io(err.to_string())
    }
}
