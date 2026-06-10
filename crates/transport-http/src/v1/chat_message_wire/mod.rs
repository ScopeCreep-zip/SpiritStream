//! Wire-mirror types for [`spiritstream_core::models::ChatMessage`] and
//! its dependency tree — utoipa is transport-only, so `ToSchema` lives
//! on these mirrors rather than the core types.
//!
//! G5: replaces the `Vec<serde_json::Value>` return on
//! `v1_chat_search_session_proxy` with the fully-typed message tree.
//! Wire shape stays byte-identical to the ts-rs export at
//! `@spiritstream/types/ChatMessage` and friends.
//!
//! Reuses [`crate::v1::ChatPlatformWire`] for the platform enum (already
//! defined in `v1/chat/wire.rs`). `MessageFlags` is a bitflags `u64` on
//! the core side and a plain `u64` on the wire — matches the existing
//! ts-rs export shape (`flags: number`).
//!
//! Orchestrator split (LOC ceiling): one cohesive concern per submodule,
//! original public surface re-exported unchanged.

mod atoms;
mod author;
mod events;
mod fragments;
mod message;

pub use atoms::*;
pub use author::*;
pub use events::*;
pub use fragments::*;
pub use message::*;
