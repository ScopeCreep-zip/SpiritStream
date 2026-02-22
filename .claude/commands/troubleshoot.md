---
description: Diagnose common issues
allowed-tools:
  - Bash
  - Read
  - Glob
  - Grep
argument-hints: "issue type (build, ffmpeg, server, frontend, capture, go2rtc)"
---

Diagnose and troubleshoot common issues based on the type provided:

## Build Issues
- Check if node_modules exists: `ls apps/web/node_modules`
- Check pnpm workspace: `pnpm ls --depth 0`
- Check Rust toolchain: `rustc --version && cargo --version`
- Run clean build: `pnpm build`
- Check Rust build: `cargo build --manifest-path server/Cargo.toml`
- Verify turbojpeg: `pkg-config --libs libturbojpeg`

## FFmpeg Issues
- Check if FFmpeg is in PATH: `which ffmpeg && ffmpeg -version`
- Check encoder support: `ffmpeg -encoders | grep h264`
- Check go2rtc FFmpeg source config in server logs
- Review FFmpeg stderr output for encoding errors

## Server Issues
- Check if server is running: `curl http://localhost:8008/health`
- Check server logs for startup errors
- Verify port 8008 is not in use: `lsof -i :8008`
- Check go2rtc is accessible: `curl http://localhost:1984/api/streams`

## Frontend Issues
- Check Vite dev server: `curl http://localhost:5173`
- Check for TypeScript errors: `pnpm typecheck`
- Check backend connection: look at browser console for WebSocket errors
- Verify CORS headers from backend

## Capture Issues
- Check macOS permissions: screen recording, camera, microphone
- Look for "permission denied" or "not authorized" in server logs
- Check scap/nokhwa device enumeration
- Verify VideoToolbox availability: `ffmpeg -hide_banner -encoders | grep videotoolbox`

## go2rtc Issues
- Check go2rtc process: `ps aux | grep go2rtc`
- Check stream registration: `curl http://localhost:1984/api/streams`
- Check WebRTC connections: `curl http://localhost:1984/api/webrtc`
- Look for "wrong sync byte" (MPEG-TS alignment issue)

Provide:
1. Diagnosis of the problem
2. Root cause analysis
3. Step-by-step fix instructions
