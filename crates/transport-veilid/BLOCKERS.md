# Veilid transport — contract gaps the real implementation must close

This is the Veilid contract-validation spike deliverable. Each item is
something that the current SpiritStream contract assumes about an HTTP
transport but that a Veilid (DHT-routed RPC) transport cannot satisfy
as-is. Resolving each gap is post-rewrite work; this list scopes it.

## 1. Cookie-keyed sessions

`SessionCookieMode` in `crates/transport-http/src/lib.rs` encodes
`Secure`/`SameSite`/`HttpOnly` policy. Veilid has no cookies — peer
identity is a keypair, not a browser-issued credential. The
`Transport` trait stays unchanged but the *auth subject* concept
needs to live one layer above transport.

**Fix**: introduce a `SessionStore` abstraction in `crates/core` that
caches `(subject, established_at)` keyed on whatever the transport
hands it (cookie for HTTP, public-key fingerprint for Veilid).
`IdentityProvider` (already in `crates/core/src/traits/identity.rs`)
gains a `VeilidKeypairIdentity` impl. Stub variants for both are
already enumerated in `Credential::{BearerToken, OAuthAccessToken,
VeilidKeypair}`.

## 2. CSRF middleware

`csrf_middleware` reads `Sec-Fetch-Site` and `Origin` headers — both
HTTP-specific. Veilid has no equivalent. The relevant threat (cross-
origin forgery from a malicious app) does exist in a DHT context but
its shape is different — there it's "a hostile peer crafts a request
with a stolen keypair." The mitigations diverge.

**Fix**: keep CSRF as a transport-http concern. Veilid uses
peer-identity verification as the equivalent guard. Document in
`/Users/kali/SpiritStream/.claude/rules/architecture.md` that
"per-transport security hardening" is allowed even though business
logic isn't.

## 3. URL paths in `/api/v1/*`

Everything in the typed REST surface is structured around URL paths
(`/api/v1/profiles`, `/api/v1/streams/groups/:group_id`). Veilid RPC
uses method names, not paths. The `utoipa` annotations on every
handler in `crates/transport-http/src/v1.rs` are HTTP-only by
definition.

**Fix**: the API surface needs a method-namespace layer that both
transports satisfy. Concretely: each handler in `v1.rs` would be
re-exposed as a `Method::Profile(ProfileOp)` enum in `crates/core`
that both transports invoke. The HTTP transport maps URLs to that
enum; Veilid maps RPC method names to it. This is the largest single
piece of post-rewrite work the spike identified.

## 4. Rate limiting on auth subject

`EndpointRateLimiters` keys on the SHA-256 prefix of the
cookie/Bearer-token bytes. Veilid would key on the peer's public
key. The mechanism (`governor::RateLimiter<String, ...>`) is fine —
only the key derivation differs.

**Fix**: pass the auth subject up as `&str` from the transport. Both
transports compute their own subject and hand it to the same
`RateLimiter`. Already shaped this way today; just confirming.

## 5. WebSocket-style server push (`/api/v1/events`)

The current event subscriber is a WebSocket. Veilid has a pub/sub
primitive (DHT watch routes) that's similar in shape but with
different semantics — push-only, no inline request/response. The
`EventSink` trait already in `crates/core/src/traits/event_sink.rs`
abstracts this cleanly; the gap is in the transport-http
WebSocket-specific handler.

**Fix**: extract a `TransportEventChannel` trait so HTTP supplies a
WebSocket-backed implementation and Veilid supplies a watch-route
implementation. `EventBus` (the existing in-process broadcast)
stays unchanged.

## 6. Streaming responses (audit log, OAuth flows)

The audit-log endpoint paginates JSON arrays; the OAuth-flow
endpoint redirects. Both shapes are HTTP-native. Veilid would
chunk-stream the same payloads over a DHT pipe.

**Fix**: handlers in `transport-http` are already pure functions
that return `Json<T>` — the Veilid transport would call the same
functions and serialise differently. No core change needed.

## 7. Cloud-mode startup guard

`enforce_cloud_mode_preconditions` checks for `BEHIND_TLS_PROXY=1`.
Veilid has no equivalent — the transport itself is end-to-end
encrypted. The guard would need a Veilid-mode variant that checks
"keypair set up" instead.

**Fix**: the guard becomes a `TransportPreconditions` trait that each
transport satisfies in its own terms. HTTP requires TLS + strong
token; Veilid requires a keypair file under `<app_data_dir>`.

## 8. Audit log HMAC chain

Independent of transport — the chain is on the on-disk audit log,
not on the wire. No blocker.

## 9. Confirm-token flow for destructive ops

`X-Confirm-Token` is an HTTP header. Veilid would carry the token
in an RPC argument. The `ConfirmTokenService` itself doesn't care
about transport.

**Fix**: handler signatures in `transport-http` already pull the
token out of headers and pass it to `state.confirm_tokens.consume`.
A Veilid handler would pull it from the method args instead.

## 10. CORS / Origin allow-list

HTTP-only by definition. Veilid has no equivalent (peers identify
by keypair, not origin). No core change.

## Summary

The biggest piece of post-rewrite work the spike identified is **#3
URL paths → method namespace**. Everything else is either:

- already abstracted (CSRF, rate-limit subjects, event-channel,
  audit-log HMAC, confirm-token, CORS),
- requires a small trait split (session store, transport
  preconditions), or
- is fundamentally transport-specific and stays that way.

The `Transport` trait itself (`serve` + `shutdown`) is correct as
written. The build succeeds with a non-HTTP impl, proving the
"transport-agnostic core" claim. The remaining work is in the layer
*between* the trait and the typed handlers — not in either side.

When the real Veilid implementation lands (post-rewrite branch), it
should:

1. Add the `Method::*` enum + handler dispatcher in `crates/core`.
2. Migrate `transport-http`'s `v1.rs` handlers to call the dispatcher
   instead of being the endpoint.
3. Implement `VeilidTransport::serve` against the same dispatcher.
4. Drop this stub crate's `NotImplemented` body for the real DHT
   integration.

Until then, the value of this spike is on disk in this document.
