// Async Timeout Helpers
//
// Replaces the duplicated spawn_blocking + timeout pattern found in
// screen_capture.rs (list_displays_async, list_windows_async) and
// other services that need blocking operations with timeout protection.

use std::time::Duration;

/// Spawn a blocking operation with a timeout.
///
/// Wraps `tokio::task::spawn_blocking` with `tokio::time::timeout` and
/// handles both panic and timeout cases with logging. Returns `None` on
/// failure instead of propagating errors — callers typically want a
/// fallback empty result.
///
/// # Arguments
/// * `label` — human-readable name for log messages (e.g., "list_displays")
/// * `timeout_secs` — maximum seconds to wait
/// * `f` — blocking function to execute
///
/// # Example
/// ```ignore
/// let displays = spawn_blocking_with_timeout("list_displays", 5, || {
///     ScreenCaptureService::list_displays()
/// }).await.unwrap_or_default();
/// ```
pub async fn spawn_blocking_with_timeout<F, T>(
    label: &str,
    timeout_secs: u64,
    f: F,
) -> Option<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let future = tokio::task::spawn_blocking(f);

    match tokio::time::timeout(Duration::from_secs(timeout_secs), future).await {
        Ok(Ok(result)) => Some(result),
        Ok(Err(_join_error)) => {
            log::warn!("{} task panicked", label);
            None
        }
        Err(_elapsed) => {
            log::warn!("{} timed out after {} seconds", label, timeout_secs);
            None
        }
    }
}

/// Spawn a blocking operation with a timeout, returning a default on failure.
///
/// Convenience wrapper around [`spawn_blocking_with_timeout`] that returns
/// `T::default()` instead of `None` on failure.
pub async fn spawn_blocking_with_timeout_or_default<F, T>(
    label: &str,
    timeout_secs: u64,
    f: F,
) -> T
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + Default + 'static,
{
    spawn_blocking_with_timeout(label, timeout_secs, f)
        .await
        .unwrap_or_default()
}
