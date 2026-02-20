/// Shared utilities for spawning and managing child processes.
///
/// Consolidates repeated patterns across services:
/// - Windows console window hiding
/// - Graceful kill-then-wait shutdown

use std::process::{Child, Command};

/// Windows `CREATE_NO_WINDOW` flag to prevent console popups from spawned processes.
#[cfg(windows)]
pub const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Apply platform-specific flags to hide the console window on Windows.
///
/// On non-Windows platforms this is a no-op.
#[cfg(windows)]
pub fn configure_hidden_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

/// Apply platform-specific flags to hide the console window on Windows.
///
/// On non-Windows platforms this is a no-op.
#[cfg(not(windows))]
pub fn configure_hidden_window(_cmd: &mut Command) {
    // No-op on non-Windows platforms
}

/// Apply platform-specific flags to hide the console window on Windows (tokio variant).
///
/// On non-Windows platforms this is a no-op.
#[cfg(windows)]
pub fn configure_hidden_window_tokio(cmd: &mut tokio::process::Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

/// Apply platform-specific flags to hide the console window on Windows (tokio variant).
///
/// On non-Windows platforms this is a no-op.
#[cfg(not(windows))]
pub fn configure_hidden_window_tokio(_cmd: &mut tokio::process::Command) {
    // No-op on non-Windows platforms
}

/// Kill a child process and wait for it to fully exit.
///
/// Both the kill and wait calls are best-effort (errors are silently ignored)
/// because the process may have already exited on its own.
pub fn kill_and_wait(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}
