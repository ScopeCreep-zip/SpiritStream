# Chat OAuth Setup (Maintainers)

[Documentation](../README.md) > [Tutorials](./README.md) > Chat OAuth Setup

---

This guide covers registering the OAuth applications that power the "Login with …" buttons in the chat settings panel, and how those credentials reach the app.

End users of **official releases** never register anything — releases embed maintainer-registered client IDs at build time, the same model used by Chatterino, Firebot, OBS Studio, and SAMMI.

**Self-builders set everything up inside the app**: each platform card in Tools → Chat platforms shows a **Set up sign-in** form when the provider is unconfigured. It links to the provider's developer portal (the registration walkthroughs below), takes the Client ID (and secret where the provider requires one), and stores them encrypted on the device — no env files, no rebuild, survives restarts. The only steps outside the app are on the provider's own site.

---

## How credentials reach the app

Resolution order per provider (first real value wins):

1. **In-app setup** — the "Set up sign-in" form (or `spiritstream-cli oauth config set-provider`); persisted through the encrypted secret store.
2. **Runtime env override** — `SPIRITSTREAM_<PROVIDER>_CLIENT_ID` / `_CLIENT_SECRET` (see `.env.example`). For development and CI.
3. **Embedded at compile time** — `SPIRITSTREAM_EMBEDDED_<PROVIDER>_CLIENT_ID` / `_CLIENT_SECRET` read with `option_env!` in `crates/core/src/services/oauth/config.rs`. The release workflow injects these from GitHub repo secrets.
4. **Placeholder** — the provider reports unconfigured; `GET /api/v1/oauth/config` reports it and every flow attempt fails with the typed `oauth_provider_not_configured` error before any browser opens.

What counts as "configured":

| Provider | Needs | Flow |
|---|---|---|
| Twitch | client ID only (Public client) | Device Code Flow |
| YouTube (Google) | client ID + secret | Loopback redirect (`http://localhost:<random port>`) |
| Kick | client ID + secret | Loopback redirect (port 8891–8895) |
| Trovo | client ID + secret | Loopback redirect (port 8891–8895) |
| Facebook | client ID + secret | Loopback redirect; manual Page-Token path works without OAuth |

## Twitch — Public client, Device Code Flow

1. Create the app at <https://dev.twitch.tv/console/apps/create>.
2. Category: Application Integration. **Client type: Public.** The Device Code Flow uses no redirect URI, but the console form still requires one — enter **`http://localhost:3000`** (bare `http://localhost` with no port trips the console's "Redirect URIs must use HTTPS protocol" check; the localhost HTTP exception only applies when a port is present). Fill the single field and Save — don't click "Add" to create a second, empty field, which also fails validation. The value is never used.
3. Copy the Client ID. **Do not generate or embed a client secret** — SpiritStream treats Twitch as a public client: sign-in runs the [Device Code Flow](https://dev.twitch.tv/docs/authentication/getting-tokens-oauth/#device-code-grant-flow) (user enters a short code at `twitch.tv/activate`), and token refresh omits the secret and persists the rotated refresh token on every cycle.
4. Scopes requested: `chat:read chat:edit moderator:manage:chat_settings` (the last one powers the follower-only-default safety feature).

Setting `SPIRITSTREAM_TWITCH_CLIENT_SECRET` switches Twitch to the confidential loopback code flow — only do that with an app registered as **Confidential**.

## YouTube — Google Desktop-app client

1. In <https://console.cloud.google.com/>, create (or pick) a project and enable **YouTube Data API v3**.
2. Under *APIs & Services → Credentials*, create an **OAuth client ID** of type **Desktop app**. Desktop clients use the loopback redirect (`http://localhost:<port>`) — no redirect URI registration, any port works.
3. Copy the client ID and secret. Google's own docs state the installed-app secret is "not treated as a secret"; embedding it in release builds is the sanctioned pattern.
4. Scope requested: `https://www.googleapis.com/auth/youtube.force-ssl`. This is a **sensitive scope**: until the OAuth consent screen passes Google's verification, the app is capped at 100 test users and shows an "unverified app" warning. Start verification early; document the cap in release notes until it clears.

## Kick

1. Register at <https://kick.com/settings/developer>.
2. Add redirect URIs for the loopback callback. SpiritStream tries ports **8891–8895**, so register all five: `http://localhost:8891/oauth/callback` … `http://localhost:8895/oauth/callback`. If the dashboard only accepts one, register 8891 — the callback server prefers the lowest free port.
3. Kick has **no public-client option**: the client secret is required at token exchange. Copy both values.
4. Scopes requested: `user:read chat:write`.

## Trovo

1. Apply for an application on the Trovo Open Platform (<https://developer.trovo.live/>). Approval is manual and can take a few days.
2. Register the loopback redirect exactly as Kick above (`http://localhost:8891/oauth/callback`; exact-match).
3. Copy the client ID and secret. The same client ID also serves the read-only channel chat token, so a Trovo ID without a secret still enables chat reading — the secret adds sign-in + send.
4. Scopes requested: `user_details_self chat_send_self send_to_my_channel`.

## Facebook

1. Create a Meta app at <https://developers.facebook.com/apps/> (type: Business).
2. Add the *Facebook Login* product; enable the loopback redirect (`http://localhost:8891/oauth/callback`) under *Valid OAuth Redirect URIs*.
3. Scopes requested by the OAuth flow: `publish_video pages_read_engagement pages_manage_posts pages_show_list`. **App Review gates all of them** for anyone outside the app's developer/tester roles — production Facebook OAuth requires a completed review with screencasts.
4. Until review clears (and for most self-builders), the UI's manual **Page Access Token** path is the realistic option: generate a long-lived Page token in the [Graph API Explorer](https://developers.facebook.com/tools/explorer/) with the two scopes above and paste it into the Facebook card in chat settings. The identity warning and server-side confirm-token gate apply either way.

The Graph API version is pinned in one place: `FACEBOOK_GRAPH_VERSION` in `crates/core/src/services/oauth/provider.rs` (currently v24.0). Bumping it updates the auth dialog, token, revoke, and comments endpoints together.

## Wiring release builds

`.github/workflows/release.yml` passes these repo secrets into the build environment as compile-time vars:

| Repo secret | Build-time env |
|---|---|
| `EMBEDDED_TWITCH_CLIENT_ID` | `SPIRITSTREAM_EMBEDDED_TWITCH_CLIENT_ID` |
| `EMBEDDED_YOUTUBE_CLIENT_ID` / `_SECRET` | `SPIRITSTREAM_EMBEDDED_YOUTUBE_CLIENT_ID` / `_CLIENT_SECRET` |
| `EMBEDDED_KICK_CLIENT_ID` / `_SECRET` | `SPIRITSTREAM_EMBEDDED_KICK_CLIENT_ID` / `_CLIENT_SECRET` |
| `EMBEDDED_FACEBOOK_CLIENT_ID` / `_SECRET` | `SPIRITSTREAM_EMBEDDED_FACEBOOK_CLIENT_ID` / `_CLIENT_SECRET` |
| `EMBEDDED_TROVO_CLIENT_ID` / `_SECRET` | `SPIRITSTREAM_EMBEDDED_TROVO_CLIENT_ID` / `_CLIENT_SECRET` |

Unset secrets are fine — the corresponding provider ships unconfigured and the UI says so.

## Verifying a build

```bash
# Per-provider setup summaries (configured, needsSecret, override id):
spiritstream-cli oauth config get

# Store one provider's credentials (CLI mirror of the in-app form;
# persists across invocations — the secret rides stdin, never argv):
printf '%s' "$KICK_SECRET" | spiritstream-cli oauth config set-provider kick \
  --client-id <id> --client-secret-from stdin

# Device flow end-to-end (prints code + URL, polls, persists):
spiritstream-cli oauth device twitch --profile <name>

# Loopback flow for the secret-bearing providers:
spiritstream-cli oauth start kick
```

In the app: every chat-platform card shows either a working sign-in button or the **Set up sign-in** form — never a dead link.
