# Environment Variables

## Frontend (Vite)

| Variable | Default | Description |
|----------|---------|-------------|
| `VITE_BACKEND_MODE` | auto-detect | Force `http` mode (auto-detects Tauri vs browser) |
| `VITE_BACKEND_URL` | `http://localhost:8008` | Backend URL for HTTP mode |
| `VITE_BACKEND_TOKEN` | — | Auth token for backend API |

## Backend Server

| Variable | Default | Description |
|----------|---------|-------------|
| `SPIRITSTREAM_HOST` | `127.0.0.1` | Bind address (localhost = no remote access) |
| `SPIRITSTREAM_PORT` | `8008` | HTTP server port |
| `SPIRITSTREAM_API_TOKEN` | — | Auth token (optional, enforced when set) |
| `SPIRITSTREAM_UI_ENABLED` | `0` | Serve static UI files from backend |

## Build Environment (macOS)

| Variable | Value | Required For |
|----------|-------|-------------|
| `PKG_CONFIG_PATH` | `/usr/local/Cellar/jpeg-turbo/3.1.3/lib/pkgconfig:$PKG_CONFIG_PATH` | turbojpeg JPEG encoding |

Prerequisites: `brew install jpeg-turbo cmake`
