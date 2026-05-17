# Rust crates

This directory holds the SpiritStream Rust workspace members that make up the rewritten core.

| Crate | Purpose |
|---|---|
| `core/` | `spiritstream-core` — transport-agnostic business logic. No Axum, no Tauri, no HTTP. |
| `transport-http/` | `HttpTransport` — Axum + utoipa, versioned REST under `/api/v1/*`. |
| `transport-cli/` | `spiritstream-cli` binary — in-process dispatch into `spiritstream-core`. First-class headless client and test substrate. |
| `transport-veilid/` | Spike stub validating the `Transport` contract; full Veilid integration is future work. |

Each transport adapter implements the `Transport` trait defined in `core::traits::transport`, takes a `ServiceRegistry`, and exposes the same operations through its own protocol. If a transport requires a special case to be expressed in core, that is a contract bug — fix core, not the transport.

See `/Users/kali/SpiritStream/.claude/rules/architecture.md` for the layered architecture.
