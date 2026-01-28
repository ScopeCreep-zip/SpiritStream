#!/bin/bash
# Kill any running SpiritStream server and related processes

echo "Stopping SpiritStream processes..."

# Kill server processes
pkill -f spiritstream-server 2>/dev/null
if [ $? -eq 0 ]; then
  echo "  ✓ Server processes terminated"
else
  echo "  - No server processes found"
fi

# Kill orphaned FFmpeg thumbnail/preview processes from our app
pkill -9 -f "ffmpeg.*avfoundation.*vframes 1" 2>/dev/null
if [ $? -eq 0 ]; then
  echo "  ✓ FFmpeg preview processes terminated"
else
  echo "  - No FFmpeg preview processes found"
fi

echo "Done."
