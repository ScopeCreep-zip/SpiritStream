# Security policy

SpiritStream serves people whose safety can depend on the app behaving
exactly as it claims to: adult and sex-work creators, harassment-prone
streamers, trans/queer/journalist communities, and other users who
cannot afford a leak. A vulnerability in SpiritStream is not "just a
bug" — please treat reports accordingly.

## Reporting a vulnerability

**Do not open a public GitHub issue, pull request, or Discussion thread.**

Use GitHub **Private Vulnerability Reporting** instead:

> **[→ Open a private security advisory](https://github.com/ScopeCreep-zip/SpiritStream/security/advisories/new)**

That channel notifies the maintainers, creates a draft advisory only
visible to invited collaborators, and gives you a private comment
thread to coordinate the fix. We deliberately do not publish a single
security email so reports can't be misrouted, scraped, or spam-flooded.

If GitHub PVR is genuinely unreachable for you (e.g. you're behind a
filter that blocks it), open an empty public issue titled
`security: please contact me privately` with no detail — a maintainer
will reach out via your GitHub profile.

### What to include in the report

The faster we can reproduce, the faster we can ship a fix:

- A short description of the vulnerability and its impact.
- Affected versions (output of `spiritstream-cli --version` and any
  Tauri / web bundle commit if you know it).
- A minimum reproduction — exact commands, configuration, network
  conditions, OS and version.
- Whether the issue is exploitable by an attacker without prior access,
  by a peer in a chat / stream, or only with local code execution.
- Whether you have a proof-of-concept (please don't attach exploit
  payloads as public-issue links).

If you would like credit in the advisory, tell us how you'd like to be
named.

## Our commitments to reporters

- We acknowledge receipt within **3 business days**.
- We provide a triage assessment (accepted / not accepted, severity,
  expected fix timeline) within **10 business days**.
- We coordinate the public advisory with you — you choose when your
  name appears and at what level of detail the writeup discloses the
  root cause.
- We default to a **90-day embargo** from initial report. We will
  shorten it if a fix ships sooner, and may extend it (with your
  agreement) if exploitation in the wild is unlikely and a coordinated
  multi-party fix is needed.
- We will not initiate legal action against good-faith researchers who
  follow this policy.

## Scope

In scope:

- The `spiritstream-core` Rust library and every transport adapter
  (`transport-http`, `transport-cli`, `transport-veilid`).
- The `server` binary and the Tauri desktop / mobile shells under
  `apps/tauri/`.
- The React frontend in `apps/web/` (XSS, injection, supply-chain
  issues in our own code).
- Build and release infrastructure under `.github/workflows/`,
  `deploy/`, `scripts/`.
- Cryptography, secret storage, audit log, panic-disconnect, PII
  filter, EXIF/PII stripper, and anonymous-mode pseudonymizer code
  paths.
- Anything documented in the threat model files under
  `.claude/claudedocs/`.

Out of scope:

- Vulnerabilities in third-party services (Twitch, YouTube, Kick, etc.)
  — report to the upstream vendor.
- Vulnerabilities in the Tauri, Rust, or Node.js runtimes — report to
  the upstream project; we will track our exposure and pin / patch as
  needed once the upstream fix lands.
- Social-engineering attempts against maintainers or other
  contributors that don't depend on a software vulnerability.
- Findings from automated scanners with no proof of exploitability;
  please confirm a real impact before reporting.

## Supported versions

The development branch (`kali/rewrite` during the current rewrite, then
`main` after) is the only branch actively receiving security fixes.
Older `latest`-tagged release lines may receive a backport at our
discretion if the impact is high and the patch is small.

| Version line | Receives security fixes? |
|--------------|--------------------------|
| `main` / pre-release | ✅ always |
| Most recent `latest` release | ✅ until next minor |
| Older `latest` releases | ❌ — please upgrade |
| Rewrite preview tags (`v0.x-rc*`) | ✅ for the duration of the rewrite |

## Hardening attestations and supply-chain transparency

Every release publishes:

- **SHA256SUMS** — a single integrity manifest covering every release
  artifact (signed via Sigstore).
- **`*.sigstore`** — SLSA Build Level 3 in-toto attestation generated
  by `actions/attest-build-provenance` and logged to the Sigstore Rekor
  transparency log. Verify with `gh attestation verify`.
- **`*.cosign.sig` / `*.cosign.crt`** — Sigstore keyless signature bound
  to the GitHub Actions OIDC identity that built the artifact.
- **CycloneDX SBOM** (`spiritstream-rust.cdx.json`,
  `spiritstream-js.cdx.json`) — full direct + transitive dependency
  inventory in OWASP SCVS Level 2 format.

Reproducing or auditing those artifacts requires nothing more than the
public release page; you do not need to trust any single maintainer's
private key.

## What we don't run

We do **not** run a paid bug bounty. We will publicly credit serious
findings in the advisory; that's the entire reward, and we have shaped
this policy to reflect what works for projects of our size after
watching curl, OpenSSL, and others struggle with AI-generated bounty
spam in 2025–2026.

## Cryptography and threat-model documentation

For background on the population-specific threats we defend against
(EXIF leaks, doxxing patterns, OAuth-refresh surveillance, panic
disconnect, encrypted-at-rest secrets, audit-log tamper detection),
see the threat-model documents under `.claude/claudedocs/` and the
service catalog in `.claude/rules/architecture.md`.
