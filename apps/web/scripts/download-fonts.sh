#!/bin/bash
# Download Google Fonts (Space Grotesk + JetBrains Mono) as local woff2 files
# Run this script once to bundle fonts locally for the desktop app.
#
# Usage: bash apps/web/scripts/download-fonts.sh

set -euo pipefail

FONT_DIR="$(dirname "$0")/../public/fonts"
mkdir -p "$FONT_DIR"

echo "Fetching Google Fonts CSS..."

# Fetch the CSS with a Chrome User-Agent to get woff2 format
CSS=$(curl -sS \
  -H "User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36" \
  "https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500&display=swap")

# Extract all woff2 URLs
URLS=$(echo "$CSS" | grep -oE 'https://fonts\.gstatic\.com/[^)]+\.woff2')

for url in $URLS; do
  filename=$(basename "$url")
  echo "Downloading $filename..."
  curl -sS -o "$FONT_DIR/$filename" "$url"
done

echo ""
echo "Downloaded files:"
ls -la "$FONT_DIR"/*.woff2 2>/dev/null || echo "No woff2 files found!"

echo ""
echo "Now generating @font-face CSS from Google's CSS..."
echo ""

# Print the CSS for reference so we can verify the @font-face declarations
echo "=== Google Fonts CSS (for reference) ==="
echo "$CSS"

echo ""
echo "Done! Font files are in: $FONT_DIR"
