//! Subcommand implementations.
//!
//! Every command in the rewrite plan's CLI surface lives here. The CLI
//! binding mirrors the REST shape so the two transports stay in lockstep.

pub mod audit;
pub mod chat;
pub mod confirm_token;
pub mod data;
pub mod discord;
pub mod events;
pub mod files;
pub mod oauth;
pub mod obs;
pub mod profile;
pub mod safety;
pub mod session;
pub mod settings;
pub mod stream;
pub mod system;
pub mod theme;
