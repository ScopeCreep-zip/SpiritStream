//! Event broadcasting contract.
//!
//! The legacy `services::events::EventSink` trait already exists and is used
//! pervasively in the codebase. This module re-exports it under the canonical
//! `traits::` path so new code can depend on the contract location, not the
//! historical implementation location. Both paths resolve to the same trait.

pub use crate::services::EventSink;
