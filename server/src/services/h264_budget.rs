// H264 Hardware Encoder Budget
// Enforces a maximum number of concurrent H264 hardware encoding sessions
// to prevent exhausting platform-specific VideoToolbox/NVENC/VAAPI slots.
//
// Uses an RAII guard pattern — sessions are automatically released when dropped.
// On macOS M-series, VideoToolbox has a hard limit of ~3-4 concurrent sessions;
// exceeding this crashes or freezes the machine.

use std::sync::atomic::{AtomicUsize, Ordering};

pub struct H264Budget {
    max_sessions: usize,
    active: AtomicUsize,
}

impl H264Budget {
    pub fn new() -> Self {
        let max_sessions = Self::detect_max_sessions();
        log::info!(
            "[H264Budget] Initialized with max {} concurrent sessions",
            max_sessions
        );
        Self {
            max_sessions,
            active: AtomicUsize::new(0),
        }
    }

    /// Detect platform-specific maximum concurrent H264 hardware sessions.
    fn detect_max_sessions() -> usize {
        #[cfg(target_os = "macos")]
        {
            // M-series (ARM): ~3 concurrent VideoToolbox sessions
            // Intel Mac: ~8 concurrent Quick Sync sessions
            if cfg!(target_arch = "aarch64") {
                3
            } else {
                8
            }
        }
        #[cfg(target_os = "windows")]
        {
            8 // NVENC/QSV typically supports 8+
        }
        #[cfg(target_os = "linux")]
        {
            4 // VAAPI varies by hardware
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            4 // Conservative default
        }
    }

    /// Try to acquire an H264 encoding session slot.
    ///
    /// Returns `Some(H264BudgetGuard)` if a slot is available, `None` if budget
    /// is exhausted. The guard automatically releases the slot when dropped.
    pub fn try_acquire(&self) -> Option<H264BudgetGuard<'_>> {
        // Optimistic increment — if we exceed budget, roll back
        let prev = self.active.fetch_add(1, Ordering::SeqCst);
        if prev < self.max_sessions {
            log::debug!(
                "[H264Budget] Acquired session slot ({}/{})",
                prev + 1,
                self.max_sessions
            );
            Some(H264BudgetGuard { budget: self })
        } else {
            self.active.fetch_sub(1, Ordering::SeqCst);
            log::warn!(
                "[H264Budget] Budget exhausted ({}/{}) — refusing new session",
                self.max_sessions,
                self.max_sessions
            );
            None
        }
    }

    /// Get the number of currently active sessions.
    pub fn active_count(&self) -> usize {
        self.active.load(Ordering::Relaxed)
    }

    /// Get the maximum allowed sessions.
    pub fn max_sessions(&self) -> usize {
        self.max_sessions
    }
}

impl Default for H264Budget {
    fn default() -> Self {
        Self::new()
    }
}

/// RAII guard that releases an H264 encoding session slot when dropped.
pub struct H264BudgetGuard<'a> {
    budget: &'a H264Budget,
}

impl Drop for H264BudgetGuard<'_> {
    fn drop(&mut self) {
        let prev = self.budget.active.fetch_sub(1, Ordering::SeqCst);
        log::debug!(
            "[H264Budget] Released session slot ({}/{})",
            prev - 1,
            self.budget.max_sessions
        );
    }
}
