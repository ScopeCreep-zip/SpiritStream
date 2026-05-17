//! Clock abstraction for deterministic testing.
//!
//! Service code that touches "now" or schedules timeouts depends on this
//! trait rather than on `std::time::SystemTime` directly, so integration
//! tests can drive the clock from a CLI flag.

use chrono::{DateTime, Utc};

pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}
