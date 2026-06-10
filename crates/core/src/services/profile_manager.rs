//! Facade — preserves the historical import path
//! `crate::services::profile_manager::ProfileManager` while the real impl
//! lives in `services::profile::*`. External callers (e.g.
//! `services::profile_activation`) do not need updating.
//!
//! Internal sub-modules:
//! - `services/profile/mod.rs`         — `ProfileManager` struct + `new()` + tests
//! - `services/profile/events.rs`      — `ProfileActivatedEvent`, `ActivatedObs`
//! - `services/profile/validation.rs`  — name + settings bounds + public constants
//! - `services/profile/order_index.rs` — order-index file I/O
//! - `services/profile/io.rs`          — file I/O: list, load, delete, etc.
//! - `services/profile/security.rs`    — encryption boundary

pub use super::profile::{
    ActivatedObs, ProfileActivatedEvent, ProfileManager, BACKEND_PORT_MIN,
    DISCORD_COOLDOWN_SECONDS_MAX,
};

// Re-export the encrypted-profile magic constants for sibling modules
// (e.g. `services::encryption` tests) that still reference the historical
// `profile_manager::ENCRYPTED_MAGIC_*` path. New code should reach for
// `profile::io::ENCRYPTED_MAGIC_*` instead.
pub(crate) use super::profile::io::{ENCRYPTED_MAGIC_LEN, ENCRYPTED_MAGIC_V1, ENCRYPTED_MAGIC_V2};
