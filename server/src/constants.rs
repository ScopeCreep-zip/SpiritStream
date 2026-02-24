pub(crate) const AUTH_COOKIE_NAME: &str = "spiritstream_session";
pub(crate) const COOKIE_MAX_AGE_SECS: i64 = 7 * 24 * 60 * 60; // 7 days
pub(crate) const DEFAULT_RATE_LIMIT_PER_MINUTE: u32 = 300;

/// Commands that are called frequently for polling and don't need logging
pub(crate) const QUIET_COMMANDS: &[&str] = &[
    "get_chat_status",
    "get_platform_chat_status",
    "is_chat_connected",
    "obs_get_state",
    "obs_is_connected",
    "get_active_stream_count",
    "get_active_group_ids",
];
