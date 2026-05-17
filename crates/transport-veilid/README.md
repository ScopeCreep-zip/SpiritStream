# spiritstream-transport-veilid

**Contract-validation spike. Not a working transport.**

This crate exists to prove the `spiritstream_core::traits::Transport`
trait surface generalises beyond HTTP. It compiles against
`crates/core` alone — no Axum, no Tauri, no Veilid SDK. If the build
succeeds, the abstraction holds; if it doesn't, `crates/core` has
accidentally grown an HTTP-shaped assumption that needs fixing.

## What this crate is NOT

* Not a production dependency. `server/` and `apps/tauri/` link only
  `transport-http`.
* Not a working transport. `VeilidTransport::serve` returns
  `CoreError::NotImplemented`. No DHT routing, no keypair identity,
  no async-rt-tied loop.

## The actual deliverable

[`BLOCKERS.md`](./BLOCKERS.md) — 10 enumerated HTTP-shaped contract
gaps a real Veilid implementation must close. The largest is
**#3: URL paths → method namespace** (every typed REST handler in
`v1.rs` routes by URL; a real DHT transport needs a `Method::*` enum
+ dispatcher both transports share).

## Tests

```bash
cargo test -p spiritstream-transport-veilid
```

One contract-validation test asserts the `Transport` trait is
satisfied. **The value is that this file compiles** against
`crates/core` alone.
