# Deferred work

The whole `kali/rewrite` SpiritStream branch ships with **one** deliberately deferred item. Anything that doesn't appear in this file is in scope; if you're tempted to defer something, surface it for a decision rather than adding a row here.

## Z1. Veilid transport — full implementation

**State**: contract-validation spike only. The `transport-veilid` crate compiles, satisfies the core `Transport` trait, and `serve()` returns `CoreError::NotImplemented`. The deliverable so far is `crates/transport-veilid/BLOCKERS.md` — an enumeration of 10 HTTP-shaped contract gaps the real implementation must close (URL paths → method namespace, cookie-keyed sessions, etc).

**Unblock condition**: every one of the following has to land before promotion past the spike:

1. **Veilid 1.0 stable release.** Pre-1.0 wire format / DHT routing semantics shift; building production transport against a moving target produces silent breakage.
2. **Audited Rust bindings** (`veilid-core` reaching a stable-API state with an independent security review). The transport touches every secret-bearing wire that the HTTP path does; an unaudited binding is a credential-leak surface.
3. **Closure of the 10 contract gaps in `crates/transport-veilid/BLOCKERS.md`.** Each gap has a concrete migration step; promotion requires all 10 resolved.

**Why this is the only legal deferral**: every other "we'll do it later" rotted into shipped-broken state during the rewrite. Veilid sits outside that pattern because the upstream dependency objectively isn't ready and the work-blocked-on-it is a parallel transport, not a fix to an existing one. Promoting any other item into this file requires explicit user approval and a documented unblock condition with the same shape (concrete event triggers, not vibes).

**Owner**: maintainer (no agent should attempt to ship Veilid without the user lifting the deferral).

**Tracker**: `crates/transport-veilid/BLOCKERS.md` keeps the contract-gap enumeration up to date; this file is the single-page "is X deferred?" lookup.
