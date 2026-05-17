//! # spiritstream-core
//!
//! Transport-agnostic business logic for SpiritStream. This crate must NOT
//! depend on `axum`, `tower`, `tauri`, or any other transport framework —
//! see `.claude/rules/architecture.md`.
//!
//! Transport adapters (HTTP, CLI, Tauri command bridge, future Veilid) live
//! in sibling crates and consume the services exported here.

pub mod commands;
pub mod errors;
pub mod models;
pub mod registry;
pub mod services;
pub mod traits;

// Re-export the structured error enum at the crate root for ergonomic use.
pub use errors::CoreError;
pub use registry::{NoopEventSink, ServiceRegistry, ServiceRegistryOptions};
