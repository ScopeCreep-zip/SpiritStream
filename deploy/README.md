# Deployment artifacts

Production deployment configurations for the three supported runtime topologies. The existing top-level `docker/` directory still holds the canonical Dockerfile during the rewrite; this directory will absorb it.

| Directory | Purpose |
|---|---|
| `docker/` | Image build context for the standalone server (Docker + browser-served UI). |
| `compose/` | `docker-compose.yml` for personal self-hosted single-user deploys, with reverse proxy + Let's Encrypt. |
| `helm/` | Helm chart for Kubernetes deploys with hardened SecurityContext. |

Cloud mode refuses to start without TLS and a strong `SPIRITSTREAM_API_TOKEN`. Remote access defaults off until the user explicitly enables it. Multi-tenant SaaS is deliberately not supported — SpiritStream's threat model assumes a single trusted operator per instance.
