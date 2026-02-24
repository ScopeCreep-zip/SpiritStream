pub const AUTH_COOKIE_NAME: &str = "spiritstream_session";
pub const COOKIE_MAX_AGE_SECS: i64 = 7 * 24 * 60 * 60; // 7 days
pub const DEFAULT_RATE_LIMIT_PER_MINUTE: u32 = 300;

/// Commands that are called frequently for polling and don't need logging
pub const QUIET_COMMANDS: &[&str] = &[
    #[cfg(feature = "chat")]
    "get_chat_status",
    #[cfg(feature = "chat")]
    "get_platform_chat_status",
    #[cfg(feature = "chat")]
    "is_chat_connected",
    #[cfg(feature = "obs")]
    "obs_get_state",
    #[cfg(feature = "obs")]
    "obs_is_connected",
    "get_active_stream_count",
    "get_active_group_ids",
];

// ── Server defaults ─────────────────────────────────────────────────
pub const DEFAULT_HOST: &str = "127.0.0.1";
pub const DEFAULT_PORT: u16 = 8008;

// ── FFmpeg: Reconnection ────────────────────────────────────────────
pub const RECONNECT_MAX_RETRIES: u32 = 5;
pub const RECONNECT_INITIAL_DELAY_SECS: u64 = 5;
pub const RECONNECT_MAX_DELAY_SECS: u64 = 120;

// ── FFmpeg: Process control ─────────────────────────────────────────
pub const FFMPEG_STOP_GRACE_PERIOD_SECS: u64 = 2;
pub const FFMPEG_STOP_POLL_INTERVAL_MS: u64 = 100;

// ── FFmpeg: UDP metering ────────────────────────────────────────────
pub const METER_READ_TIMEOUT_MS: u64 = 250;
pub const METER_UDP_BUFFER_SIZE: usize = 2048;

// ── FFmpeg: RTMP options ────────────────────────────────────────────
pub const RTMP_RECV_TIMEOUT_US: &str = "30000000";
pub const RTMP_CLIENT_BUFFER_MS: &str = "30000";

// ── FFmpeg: Stats ───────────────────────────────────────────────────
pub const STATS_EMIT_INTERVAL_MS: u64 = 1000;
pub const STATS_RECENT_LINES_CAPACITY: usize = 40;
pub const STATS_BITRATE_SMOOTHING_ALPHA: f64 = 0.2;

// ── Chat ────────────────────────────────────────────────────────────
#[cfg(feature = "chat")]
pub const CHAT_MAX_SEEN_IDS: usize = 5000;
#[cfg(feature = "chat")]
pub const CHAT_OUTBOUND_DEDUP_WINDOW_SECS: u64 = 10;

// ── Chat lifecycle/reconnect ────────────────────────────────────────
#[cfg(feature = "chat")]
pub const CHAT_RECONNECT_INTERVAL_SECS: u64 = 15;
#[cfg(feature = "chat")]
pub const CHAT_RECONNECT_COOLDOWN_SECS: u64 = 30;

// ── OAuth ───────────────────────────────────────────────────────────
pub const OAUTH_CALLBACK_PORT_START: u16 = 49152;
pub const OAUTH_CALLBACK_PORT_END: u16 = 49162;
pub const OAUTH_FLOW_TIMEOUT_SECS: u64 = 600;

// ── Token refresh ───────────────────────────────────────────────────
#[cfg(feature = "chat")]
pub const TOKEN_REFRESH_INTERVAL_SECS: u64 = 60;
