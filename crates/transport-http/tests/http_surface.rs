//! End-to-end integration test: boot the real `spiritstream-server` binary
//! on a free port against a temp data dir, exercise the `/api/v1/*` surface,
//! and confirm the OpenAPI document is well-formed.
//!
//! These tests pin the HTTP transport's verification criteria from the rewrite plan:
//!   - Every route SpiritStream serves lives under `/api/v1/*` — no legacy
//!     aliases. Unversioned URLs (`/health`, `/ready`, `/auth/*`, `/ws`,
//!     `/api/files/*`, `/api/invoke/*`) return 404.
//!   - The OpenAPI document at `/api/v1/openapi.json` parses and lists every
//!     typed handler the v1 module registers.
//!   - The transitional `POST /api/v1/invoke/:command` dispatch bridge works
//!     until it is retired.
//!
//! `spiritstream-cli` and CLI-driven golden tests under
//! `tests/integration/` at the workspace root are the test substrate
//! for service behaviour. This test stays focused on the HTTP transport.
//!
//! K1 (Sprint K): split into per-domain modules under
//! `tests/http_surface/` so each file lives well under the 600 LOC
//! ceiling. Shared boot + HTTP-client helpers live in `common.rs`.

#[path = "http_surface/common.rs"]
mod common;

#[path = "http_surface/system.rs"]
mod system;

#[path = "http_surface/profiles.rs"]
mod profiles;

#[path = "http_surface/streams.rs"]
mod streams;

#[path = "http_surface/settings.rs"]
mod settings;

#[path = "http_surface/csrf.rs"]
mod csrf;

#[path = "http_surface/auth.rs"]
mod auth;
