#!/bin/bash
# Kill any running SpiritStream server and related processes

echo "Stopping SpiritStream processes..."

# Kill by process name (force kill with -9)
pkill -9 -f spiritstream-server 2>/dev/null && echo "  ✓ spiritstream-server" || echo "  - No spiritstream-server"
pkill -9 -f spiritstream-desktop 2>/dev/null && echo "  ✓ spiritstream-desktop" || echo "  - No spiritstream-desktop"
pkill -9 -f go2rtc 2>/dev/null && echo "  ✓ go2rtc" || echo "  - No go2rtc"

# Kill orphaned FFmpeg processes from our app
pkill -9 -f "ffmpeg.*avfoundation" 2>/dev/null && echo "  ✓ FFmpeg preview processes" || echo "  - No FFmpeg preview"
pkill -9 -f "ffmpeg.*rtmp://127.0.0.1" 2>/dev/null && echo "  ✓ FFmpeg stream processes" || echo "  - No FFmpeg stream"

# Kill Vite dev server and related node processes
pkill -9 -f "vite.*spiritstream" 2>/dev/null && echo "  ✓ Vite dev server" || echo "  - No Vite"

# Kill anything on our ports
for port in 1420 8008 1984; do
  pid=$(lsof -ti :$port 2>/dev/null)
  if [ -n "$pid" ]; then
    kill -9 $pid 2>/dev/null && echo "  ✓ Process on port $port (PID $pid)"
  fi
done

# Wait for ports to be released
sleep 0.5

# Verify cleanup
echo ""
echo "Port status:"
for port in 1420 8008 1984; do
  if lsof -i :$port >/dev/null 2>&1; then
    echo "  ⚠ Port $port still in use"
  else
    echo "  ✓ Port $port is free"
  fi
done

echo ""
echo "Done."
