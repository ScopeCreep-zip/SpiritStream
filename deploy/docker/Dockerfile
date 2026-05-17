# =============================================================================
# SpiritStream Backend — Docker Build.
#
# Multi-stage workspace build. Base: Debian bookworm-slim for FFmpeg
# compatibility on the runtime side; pin the build image at or above
# the workspace's rust-version (1.77.2 today; bump in lockstep with
# `Cargo.toml [workspace.package].rust-version`).
# =============================================================================

# -----------------------------------------------------------------------------
# Stage 1 — Build environment
# -----------------------------------------------------------------------------
FROM rust:1.77-bookworm AS builder

RUN apt-get update && apt-get install -y \
        pkg-config \
        libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Workspace manifests first for dep-layer caching.
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
COPY server/ ./server/
# `data/` is read by build.rs at compile time.
COPY data ./data

# Build the release server binary against the full workspace.
RUN cargo build --release --bin spiritstream-server

# -----------------------------------------------------------------------------
# Stage 2 — Runtime environment
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y \
        ca-certificates \
        ffmpeg \
        libssl3 \
        curl \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Non-root user — required by the read-only-rootfs compose policy.
RUN groupadd -r spiritstream && useradd -r -g spiritstream spiritstream

RUN mkdir -p /app /data /data/logs /data/profiles /themes /dist \
    && chown -R spiritstream:spiritstream /app /data /themes /dist

WORKDIR /app

COPY --from=builder /build/target/release/spiritstream-server /app/spiritstream-server
COPY --chown=spiritstream:spiritstream themes/*.jsonc /themes/

RUN chmod 555 /app/spiritstream-server

USER spiritstream

# Container-appropriate defaults. The compose layer pins everything
# that matters in production (DEPLOY_MODE=cloud, BEHIND_TLS_PROXY=1,
# CORS origins, COOKIE_MODE, etc.); ENV here just covers the dev /
# bare-`docker run` shape.
ENV SPIRITSTREAM_HOST=0.0.0.0 \
    SPIRITSTREAM_PORT=8008 \
    SPIRITSTREAM_DATA_DIR=/data \
    SPIRITSTREAM_LOG_DIR=/data/logs \
    SPIRITSTREAM_THEMES_DIR=/themes \
    SPIRITSTREAM_UI_DIR=/dist \
    SPIRITSTREAM_UI_ENABLED=0
# Cloud deployments MUST set:
#   SPIRITSTREAM_DEPLOY_MODE=cloud
#   SPIRITSTREAM_BEHIND_TLS_PROXY=1
#   SPIRITSTREAM_API_TOKEN=<≥32 chars>
# The server refuses to start in cloud mode without them.

EXPOSE 8008

HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:8008/api/v1/health || exit 1

ENTRYPOINT ["/app/spiritstream-server"]
