# =============================================================================
# SpiritStream Backend Server - Docker Build
# =============================================================================
# Multi-stage build for minimal production image
# Base: Debian (bookworm-slim) for FFmpeg compatibility
# =============================================================================

# -----------------------------------------------------------------------------
# Stage 1: Build Environment
# -----------------------------------------------------------------------------
FROM rust:1.77-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy Cargo files first for dependency caching
COPY server/Cargo.toml server/Cargo.lock* ./server/
COPY server/build.rs ./server/

# Create dummy source files to build dependencies
RUN mkdir -p server/src/commands server/src/models server/src/services \
    && echo "fn main() {}" > server/src/main.rs \
    && echo "" > server/src/lib.rs \
    && echo "" > server/src/commands/mod.rs \
    && echo "" > server/src/models/mod.rs \
    && echo "" > server/src/services/mod.rs

# Copy platform data for build script
COPY data ./data

# Build dependencies (this layer will be cached)
WORKDIR /build/server
RUN cargo build --release 2>/dev/null || true

# Copy actual source code
WORKDIR /build
COPY server ./server

# Build the server binary
WORKDIR /build/server
RUN cargo build --release

# -----------------------------------------------------------------------------
# Stage 2: Runtime Environment
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies and FFmpeg
RUN apt-get update && apt-get install -y \
    ca-certificates \
    ffmpeg \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Create non-root user for security
RUN groupadd -r spiritstream && useradd -r -g spiritstream spiritstream

# Create application directories
RUN mkdir -p /app /data /data/logs /data/profiles /themes /dist \
    && chown -R spiritstream:spiritstream /app /data /themes /dist

WORKDIR /app

# Copy the built binary from builder stage
COPY --from=builder /build/server/target/release/spiritstream-server /app/spiritstream-server

# Copy default themes
COPY --chown=spiritstream:spiritstream themes/*.jsonc /themes/

# Make binary executable but read-only (security hardening)
RUN chmod 555 /app/spiritstream-server

# Switch to non-root user
USER spiritstream

# Environment variables with container-appropriate defaults
ENV SPIRITSTREAM_HOST=0.0.0.0
ENV SPIRITSTREAM_PORT=8008
ENV SPIRITSTREAM_DATA_DIR=/data
ENV SPIRITSTREAM_LOG_DIR=/data/logs
ENV SPIRITSTREAM_THEMES_DIR=/themes
ENV SPIRITSTREAM_UI_DIR=/dist
ENV SPIRITSTREAM_UI_ENABLED=0
# SPIRITSTREAM_API_TOKEN should be set at runtime for security

# Expose the server port
EXPOSE 8008

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8008/health || exit 1

# Run the server
ENTRYPOINT ["/app/spiritstream-server"]
