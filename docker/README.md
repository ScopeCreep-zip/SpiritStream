# SpiritStream Docker Distribution

This directory contains Docker configuration files for running the SpiritStream backend server in a containerized environment.

## Overview

The SpiritStream backend server provides a REST API and WebSocket interface for managing streaming profiles, controlling FFmpeg streams, and serving the optional web UI. The Docker image includes FFmpeg pre-installed for full streaming functionality.

## Quick Start

### Using Docker Compose (Recommended)

```bash
# Build and run
cd docker
docker compose up -d

# View logs
docker compose logs -f

# Stop
docker compose down
```

### Using Docker Directly

```bash
# Build the image
docker build -t spiritstream/server -f docker/Dockerfile .

# Run the container
docker run -d \
  --name spiritstream-server \
  -p 8008:8008 \
  -v spiritstream-data:/data \
  -e SPIRITSTREAM_HOST=0.0.0.0 \
  spiritstream/server
```

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `SPIRITSTREAM_HOST` | `0.0.0.0` | Server bind address (use `0.0.0.0` in containers) |
| `SPIRITSTREAM_PORT` | `8008` | Server port |
| `SPIRITSTREAM_DATA_DIR` | `/data` | Profile and settings storage directory |
| `SPIRITSTREAM_LOG_DIR` | `/data/logs` | Log files directory |
| `SPIRITSTREAM_THEMES_DIR` | `/themes` | Custom themes directory |
| `SPIRITSTREAM_UI_DIR` | `/dist` | Static UI files directory |
| `SPIRITSTREAM_UI_ENABLED` | `0` | Enable serving the web UI (`1` = enabled) |
| `SPIRITSTREAM_API_TOKEN` | (empty) | API authentication token (recommended for production) |

### Volume Mounts

| Container Path | Purpose | Required |
|----------------|---------|----------|
| `/data` | Profile storage, settings, encryption keys | Yes |
| `/data/logs` | Application logs | Recommended |
| `/themes` | Custom theme files (read-only) | Optional |
| `/dist` | Web UI static files (read-only) | Optional |

## Usage Examples

### Basic Development Setup

```bash
docker compose up
```

The server will be available at `http://localhost:8008`.

### Production with Authentication

```bash
# Set required environment variables
export SPIRITSTREAM_API_TOKEN="your-secure-token-here"

# Run with production overrides
docker compose -f docker-compose.yml -f docker-compose.prod.yml up -d
```

### With Web UI Enabled

First, build the frontend:

```bash
# From project root
npm run build
```

Then run with UI enabled:

```bash
# Using environment variable
SPIRITSTREAM_UI_ENABLED=1 docker compose up

# Or set in .env file
echo "SPIRITSTREAM_UI_ENABLED=1" >> .env
docker compose up
```

### Custom Themes

Mount your themes directory:

```bash
docker run -d \
  --name spiritstream-server \
  -p 8008:8008 \
  -v spiritstream-data:/data \
  -v /path/to/themes:/themes:ro \
  spiritstream/server
```

### Persistent Data with Local Directories

```bash
docker run -d \
  --name spiritstream-server \
  -p 8008:8008 \
  -v $(pwd)/data:/data \
  -v $(pwd)/logs:/data/logs \
  -v $(pwd)/themes:/themes:ro \
  -e SPIRITSTREAM_API_TOKEN="your-token" \
  spiritstream/server
```

## API Endpoints

### Health Check

```bash
curl http://localhost:8008/health
# Response: {"ok":true}
```

### Invoke Commands

All SpiritStream commands are available via the invoke endpoint:

```bash
# Without authentication
curl -X POST http://localhost:8008/api/invoke/get_all_profiles \
  -H "Content-Type: application/json" \
  -d '{}'

# With authentication
curl -X POST http://localhost:8008/api/invoke/get_all_profiles \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer your-token" \
  -d '{}'
```

### WebSocket Connection

Connect to `/ws` for real-time events:

```javascript
// Without authentication
const ws = new WebSocket('ws://localhost:8008/ws');

// With authentication
const ws = new WebSocket('ws://localhost:8008/ws?token=your-token');

ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  console.log('Event:', data.event, 'Payload:', data.payload);
};
```

## Building

### Build for Local Architecture

```bash
docker build -t spiritstream/server -f docker/Dockerfile .
```

### Build with Specific Tag

```bash
docker build -t spiritstream/server:v0.1.0 -f docker/Dockerfile .
```

### Multi-Platform Build (requires Docker Buildx)

```bash
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t spiritstream/server:latest \
  -f docker/Dockerfile \
  --push .
```

## Troubleshooting

### Container Won't Start

Check the logs:

```bash
docker compose logs spiritstream
# or
docker logs spiritstream-server
```

### Permission Issues with Volumes

The container runs as a non-root user (`spiritstream`). Ensure mounted directories have appropriate permissions:

```bash
# For Linux hosts
sudo chown -R 1000:1000 ./data ./themes
```

### FFmpeg Not Found

FFmpeg is pre-installed in the container. If you see FFmpeg-related errors, ensure:

1. The container is running the correct image
2. Volume mounts aren't overriding system binaries

Check FFmpeg availability:

```bash
docker exec spiritstream-server ffmpeg -version
```

### Connection Refused

Ensure:

1. The container is running: `docker ps`
2. Port mapping is correct: `-p 8008:8008`
3. `SPIRITSTREAM_HOST` is set to `0.0.0.0` (not `127.0.0.1`)

### Health Check Failing

The health check requires `curl`. If using a minimal image without curl, the health check will fail. The provided Dockerfile includes `curl` in the runtime image.

## Security Considerations

1. **API Token**: Always set `SPIRITSTREAM_API_TOKEN` in production environments
2. **Non-root User**: The container runs as a non-root user for security
3. **Read-only Mounts**: Mount themes and UI directories as read-only (`:ro`)
4. **Network Isolation**: Use Docker networks to isolate the container
5. **Resource Limits**: Set appropriate CPU and memory limits

## File Structure

```
docker/
├── Dockerfile              # Multi-stage build for the server
├── docker-compose.yml      # Development/default configuration
├── docker-compose.prod.yml # Production overrides
└── README.md               # This documentation
```

## Integration with Reverse Proxy

### Nginx Example

```nginx
upstream spiritstream {
    server spiritstream-server:8008;
}

server {
    listen 443 ssl;
    server_name stream.example.com;

    location / {
        proxy_pass http://spiritstream;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

### Traefik Labels

```yaml
services:
  spiritstream:
    labels:
      - "traefik.enable=true"
      - "traefik.http.routers.spiritstream.rule=Host(`stream.example.com`)"
      - "traefik.http.services.spiritstream.loadbalancer.server.port=8008"
```

## License

SpiritStream is licensed under GPL-3.0. See the [LICENSE](../LICENSE) file for details.
