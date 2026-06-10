# Chat Platforms

[Documentation](../README.md) > [Streaming](./README.md) > Chat Platforms

---

SpiritStream connects to live-chat on six platforms so creators can read and
(where supported) respond to chat from a single panel, independent of which
RTMP destinations a stream is fanned out to. Chat connection is **separate**
from RTMP output: a stream can target a platform without connecting to its
chat, and vice-versa.

Every connector implements the `ChatPlatform` trait
(`crates/core/src/services/chat/platform.rs`). Inbound messages are pushed onto
a bounded channel and surfaced through the unified chat panel; outbound sending
is only available on platforms with a verified send path **and** an
authenticated session.

---

## Capability matrix

| Platform | Receive | Send | Transport | Auth for send |
|----------|:-------:|:----:|-----------|---------------|
| Twitch   | ✅ | ✅ | IRC over WebSocket | OAuth (chat scope) |
| YouTube  | ✅ | ✅ | Live-chat REST polling | OAuth |
| Kick     | ✅ | ✅ | WebSocket (Pusher) | Session token |
| Facebook | ✅ | ✅ | Graph API comment polling | Page access token |
| Trovo    | ✅ | ❌ | WebSocket | — (receive-only today) |
| TikTok   | ✅ | ❌ | Reverse-engineered protobuf/WS | — (read-only by policy) |

`Receive` means the connector streams inbound chat into the panel. `Send`
means `can_send()` can return `true` and `send_message()` has a real
implementation. A platform with `Send ❌` returns a `PlatformError` from
`send_message()` and `can_send()` is always `false`, so the composer disables
the send affordance for that platform.

---

## Per-platform notes

### Twitch
Full duplex. Connects over IRC-WebSocket; `send_message()` is enabled once the
session holds an OAuth token with the chat scope (`can_send` gate). Outbound
messages are tracked in a recent-outbound buffer for echo suppression.

### YouTube
Full duplex. Inbound uses live-chat REST polling with adaptive backoff that
resets to the base interval on the first successful poll after a quota stall.
`send_message()` requires an OAuth session (`AuthMode::OAuth`); API-key-only
sessions are receive-only. The connector supports `update_token()` to swap the
access token mid-session without reconnecting.

### Kick
Full duplex. Inbound over WebSocket; sending requires a resolved send-state
(channel + token) captured at connect time.

### Facebook
Full duplex. Inbound polls Graph API comments with a `since=` cursor for
de-duplication; sending requires a Page access token. Facebook chat is an
identity-revealing surface — the UI gates connection behind an explicit
confirm-token warning before the "Sign in" control is enabled.

### Trovo
**Receive-only today.** The connector streams inbound chat, but `can_send()`
returns `false` and `send_message()` falls through to the trait default, which
returns `PlatformError::Platform("Sending messages is not supported …")`. There
is no verified outbound path wired yet.

### TikTok
**Read-only by platform policy.** TikTok publishes no official chat API; the
connector uses the community `piratetok-live-rs` crate (reverse-engineered
protobuf-over-WebSocket). No auth or API key is needed — connect by username
and receive realtime events.

Send is intentionally disabled: TikTok rejects third-party chat-send for
non-creator-app integrations, and the reverse-engineered protocol exposes no
verified send path. `can_send()` returns `false`; `send_message()` returns
`PlatformError::Platform("TikTok chat is read-only …")`.

When TikTok rotates the protobuf protocol, events stop arriving until the
upstream crate ships a fix and the dependency is bumped (a maintainer ritual,
not an autodownload). The connector surfaces the stall as
`ChatConnectionStatus::Error` with a descriptive `last_error` so the UI never
silently hides the break.

---

## TikTok and RTMP output

TikTok chat being read-only is distinct from TikTok **RTMP ingest**. TikTok
gates LIVE/RTMP access behind account eligibility (follower threshold / approved
LIVE access) and only issues a stream key to eligible accounts. SpiritStream can
fan out to TikTok's RTMP endpoint (`rtmps://live.tiktok.com/rtmp/`) when the user
supplies a valid stream key, but it cannot mint one — an ineligible account has
no key to enter, and there is no API to request one. Expect the streaming target
to be usable only for creators TikTok has already granted LIVE access.

---

## Anonymous mode

When anonymous mode is enabled, inbound chat is routed through the
pseudonymizer before it reaches the panel or the event bus, so author display
names are replaced with stable keyed-hash pseudonyms. This applies uniformly
across all six connectors — pseudonymization happens in the chat manager, not
per-connector, so adding a connector cannot accidentally bypass it.

---

## Endpoint injection

Every network endpoint a connector talks to is centralized in one struct,
`ChatEndpoints` (`crates/core/src/services/chat/endpoints.rs`), instead of being
scattered as `const` string literals across the connector files. The production
defaults (`ChatEndpoints::default()`) are the real platform URLs; the struct is
injected through the connector factory
(`ChatManager::create_platform_connector`) so a connector never reads a
hardcoded endpoint and the whole network surface can be redirected at a single
seam.

This is deliberately **not** environment-overridable. Letting an env var
redirect chat or OAuth traffic would be an exfiltration vector against the
vulnerable users SpiritStream is built for. The only non-default construction is
`ChatEndpoints::for_mock`, gated to `#[cfg(test)]`, which points every endpoint
at a local mock server for the integration harness below.

Two live endpoints are intentionally **absent** from `ChatEndpoints`, because
neither underlying crate exposes a server-address override:

- **Twitch IRC ride** — the chat socket lives inside the `twitch-irc` crate,
  which dials `irc.chat.twitch.tv` with no host override. Only Twitch's HTTP
  seams (GQL channel lookup + OAuth `validate`) are injectable.
- **TikTok** — the entire connection lives inside `piratetok-live-rs`, whose
  builder takes only a username. There is no endpoint to rebase.

## Connector integration harness

`crates/core/src/services/chat/integration_harness/` drives each
endpoint-injectable connector through a full lifecycle against a local mock
server (`wiremock` for HTTP, a one-shot `accept_async` WebSocket server for the
WS connectors): connect → receive one message → send (where the platform
supports it) → disconnect. For the WebSocket connectors (Kick, Trovo) the mock
also asserts the connector sent a `Close` frame on disconnect, proving the read
loop shut down rather than leaking a zombie task.

| Connector | Harness coverage |
|-----------|------------------|
| Kick     | Full round-trip (REST chatroom lookup → Pusher WS → receive → REST send → Close) |
| Trovo    | Full round-trip (REST token → open-chat WS AUTH → receive → Close); receive-only, no send leg |
| YouTube  | Connect → poll-receive → authenticated send → disconnect (REST only, no WS) |
| Facebook | Connect-probe → poll-receive → send → disconnect (Graph REST only, no WS) |
| Twitch   | GQL channel-not-found rejection only — the IRC ride is not mockable, so the test stops before the IRC client is constructed |
| TikTok   | No mock round-trip — the connection lives entirely inside `piratetok-live-rs`; its read-only contract is pinned in `connector_tests.rs` |

The Twitch and TikTok rows reflect the injectability limits described under
[Endpoint injection](#endpoint-injection): their sockets are owned by upstream
crates with no host override, so a hermetic round-trip is impossible. Every
other connector exercises its real wire protocol end-to-end.

---

**Related:** [Platform Registry](./05-platform-registry.md) | [Multi-Destination Streaming](./03-multi-destination.md) | [Core crate README](../../crates/core/README.md)
