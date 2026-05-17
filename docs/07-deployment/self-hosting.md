# Self-hosting SpiritStream

This guide walks through standing up a personal SpiritStream instance on
your own VPS, home server, or cloud VM. It assumes single-tenant deploy
— **multi-tenant SaaS is a different threat model the rewrite doesn't
support**, intentionally.

> ⚠️ **Threat-model note**: a cloud-hosted instance lets your provider
> subpoena disk contents. SpiritStream hardens the bytes at rest (AES-256
> GCM-SIV under a per-machine key), but the machine key lives on the
> same disk. If the legal-process risk matters for your situation,
> keep SpiritStream on a home server or a Tor onion service rather
> than a hyperscaler VM.

## What you'll need

* A Linux host with `docker`, `docker compose` v2, and `curl`. 2 vCPU
  / 1 GiB RAM is enough for one streamer; FFmpeg encodes are where
  CPU goes.
* A public hostname pointed at the host's IPv4/IPv6 (e.g., `streams.example.com`).
* TCP ports **80** and **443** open inbound for Caddy + Let's Encrypt.
* An email address for Let's Encrypt cert-renewal notifications.

## One-shot deploy (`docker compose`)

```bash
git clone https://github.com/ScopeCreep-zip/SpiritStream.git
cd SpiritStream/deploy/compose

# Generate a strong API token. Save it — you'll paste it into the UI
# the first time you connect from a browser / desktop client.
export SPIRITSTREAM_API_TOKEN="$(openssl rand -base64 32)"
export SPIRITSTREAM_HOSTNAME="streams.example.com"
export SPIRITSTREAM_ACME_EMAIL="you@example.com"

docker compose up -d
```

On first boot Caddy obtains a Let's Encrypt cert (~30 seconds). Watch:

```bash
docker compose logs -f caddy
docker compose logs -f spiritstream
```

The backend refuses to start without all three env-vars. The
cloud-mode guard reports the exact missing piece in stderr.

## What the compose ships

| Service | Role |
|---|---|
| `spiritstream` | Backend on port `8008`, never directly exposed. Read-only rootfs, drops all caps, runs as a non-root user. |
| `caddy` | Reverse proxy on `:80` / `:443`. Auto-Let's Encrypt. Redirects HTTP → HTTPS. WebSocket-aware. Terminates TLS so SpiritStream never sees plaintext from the internet. |

Two named volumes hold state:
* `spiritstream-data` — profiles, settings, audit log, OAuth tokens (encrypted under the machine key).
* `spiritstream-logs` — server log files.

## Backup and restore

The audit log + profile files are the operationally interesting state.

```bash
# Snapshot
docker run --rm -v spiritstream-data:/data -v "$PWD":/backup \
  alpine tar czf /backup/spiritstream-$(date +%F).tar.gz -C / data

# Restore — stop services first.
docker compose down
docker run --rm -v spiritstream-data:/data -v "$PWD":/backup \
  alpine tar xzf /backup/spiritstream-2026-05-15.tar.gz -C /
docker compose up -d
```

Keep these backups encrypted at rest yourself; the in-volume contents
are already encrypted, but a leaked tarball still yields the machine
key alongside the ciphertext.

## Rotating the API token

```bash
# Generate a new token.
NEW_TOKEN="$(openssl rand -base64 32)"

# Update the running container.
SPIRITSTREAM_API_TOKEN="$NEW_TOKEN" docker compose up -d --force-recreate spiritstream

# Destructive ops (clear_data, machine-key rotate, revoke-all-sessions)
# require a confirmation token. The CLI walks you through; this rotation
# is just the auth secret.
```

## Kubernetes (Helm)

```bash
cd SpiritStream/deploy/helm
kubectl create namespace spiritstream
kubectl create secret generic spiritstream-token \
  --from-literal=token="$(openssl rand -base64 32)" \
  -n spiritstream
helm install spiritstream ./spiritstream \
  --namespace spiritstream \
  --set hostname=streams.example.com \
  --set corsOrigins=https://streams.example.com \
  --set apiTokenSecretRef.name=spiritstream-token
```

The chart provisions: 2 PVCs (data + logs), a Deployment with a
hardened SecurityContext, an Ingress with cert-manager TLS, a
NetworkPolicy locking egress to HTTPS (443) + RTMP (1935), and
a Secret either inline or external (recommended).

Override the egress allow-list if your platforms use different
ports. Default policy is **deny-all-egress except the rules in
`values.yaml`**.

## What's running

* **Port 8008** — internal HTTP. Never exposed.
* **`/api/v1/health`** — per-subsystem status. `services`
  field carries `profiles`, `settings`, `themes`, `audit_log` each
  reporting `{state: ok|degraded|disconnected|tampered}`. Tampered
  audit log surfaces a red banner in the UI.
* **`/api/v1/ready`** — Kubernetes readiness probe.
* **`/api/v1/events`** — WebSocket for live stream stats / chat events.

## Confirmation tokens

Destructive operations (clear all data, rotate the machine key, revoke
all active sessions) require an `X-Confirm-Token` issued by
`POST /api/v1/security/confirm-token`. The Web UI handles this; for
scripted operations:

```bash
# 1. Issue.
TOKEN=$(curl -s -X POST https://streams.example.com/api/v1/security/confirm-token \
  -H "Authorization: Bearer $SPIRITSTREAM_API_TOKEN" \
  -H "content-type: application/json" \
  -d '{"intent":"rotate_machine_key"}' | jq -r .token)

# 2. Use within 30 seconds.
curl -X POST https://streams.example.com/api/v1/security/machine-key/rotate \
  -H "Authorization: Bearer $SPIRITSTREAM_API_TOKEN" \
  -H "X-Confirm-Token: $TOKEN"
```

## Hardening checklist for a public instance

* [ ] `SPIRITSTREAM_API_TOKEN` ≥ 32 chars, generated with `openssl rand -base64`.
* [ ] `SPIRITSTREAM_DEPLOY_MODE=cloud` set.
* [ ] `SPIRITSTREAM_BEHIND_TLS_PROXY=1` set (the compose ships this by
      default; the server refuses to start without it in cloud mode).
* [ ] `SPIRITSTREAM_CORS_ORIGINS` tightened to your actual UI host(s) —
      no wildcards.
* [ ] Firewall: only `80` and `443` open. Block `8008` at the host firewall.
* [ ] Volumes backed up daily, backups encrypted, retention policy set.
* [ ] Caddy logs shipped somewhere durable (the container's stdout is
      ephemeral). Configure `docker compose logs` rotation or a log
      shipper.
* [ ] `SPIRITSTREAM_LOG_FORMAT=json` so log aggregators can index
      `level` + `target` + `msg` cleanly. `mask_sensitive` already
      redacts tokens / stream keys / `${TOKEN}` template values at
      the logging boundary.
* [ ] OS updates applied; SpiritStream image rebuilt on Debian
      security advisories.

## What this does NOT include

* **Multi-tenant SaaS.** SpiritStream is single-tenant by design —
  one operator, one set of stream profiles, one auth identity. Adding
  tenancy is a different rewrite.
* **Telemetry / phone-home.** Off by default
  (`error_reporting_enabled`); even when on, the operator points it at
  *their own* collector. No first-party telemetry server exists.
* **Mobile builds.** Tauri 2 mobile targets land in a follow-up branch
  (research is in `docs/01-architecture/`). Distribution there is
  AltStore PAL + F-Droid + direct APK — App Store / Play Store are
  closed under 2026 content policies.

## Where to read more

* Threat model: `.claude/claudedocs/`
* Security hardening anchors: `crates/core/src/services/encryption.rs`, `crates/transport-http/src/lib.rs`
* Observability + audit log: `crates/core/src/services/audit_log.rs`
