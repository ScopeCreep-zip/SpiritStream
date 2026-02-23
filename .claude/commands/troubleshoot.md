---
description: Diagnose common issues
allowed-tools:
  - Bash
  - Read
  - Glob
  - Grep
argument-hints: "issue type (build, ffmpeg, server, stream, desktop)"
---

Diagnose and troubleshoot common issues based on the type provided:

## Build Issues
- Check if node_modules exists: `ls node_modules`
- Check pnpm workspace: `pnpm ls --depth 0`
- Run clean build: `pnpm build`
- Check Rust compilation: `cargo check --manifest-path server/Cargo.toml`
- Check for missing dependencies

## FFmpeg Issues
- Test FFmpeg availability via API: `curl http://127.0.0.1:8008/api/invoke/test_ffmpeg -X POST`
- Check FFmpeg path in settings
- Check server logs for FFmpeg errors
- Verify FFmpeg path resolution in `server/src/services/ffmpeg_handler.rs`

## Server Issues
- Check server health: `curl http://127.0.0.1:8008/health`
- Check server readiness: `curl http://127.0.0.1:8008/ready`
- Check if port 8008 is in use: `lsof -i :8008`
- Review server logs in data directory
- Check environment variables (SPIRITSTREAM_HOST, SPIRITSTREAM_PORT)

## Desktop (Tauri) Issues
- Check Tauri sidecar configuration in `apps/desktop/src-tauri/tauri.conf.json`
- Verify server binary exists in sidecar path
- Check Tauri logs for launcher errors
- Verify webview can connect to `http://127.0.0.1:8008`

## Stream Issues
- Verify incoming URL format
- Check stream targets configuration
- Review server logs for FFmpeg process errors
- Test network connectivity to RTMP targets

Provide:
1. Diagnosis of the problem
2. Root cause analysis
3. Step-by-step fix instructions
