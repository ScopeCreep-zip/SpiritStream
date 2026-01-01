---
description: Diagnose common issues
allowed-tools:
  - Bash
  - Read
  - Glob
  - Grep
argument-hints: "issue type (build, ffmpeg, electron, stream)"
---

Diagnose and troubleshoot common issues based on the type provided:

## Build Issues
- Check if node_modules exists: `ls node_modules`
- Check TypeScript version: `npx tsc --version`
- Run clean build: `npm run clean && npm run compile`
- Check for missing dependencies

## FFmpeg Issues
- Check if FFmpeg exists: `ls resources/ffmpeg/bin/`
- Test FFmpeg: `./resources/ffmpeg/bin/ffmpeg -version`
- Check encoders.conf is valid JSON
- Verify FFmpeg path resolution in code

## Electron Issues
- Check main.js exists: `ls dist/electron/main.js`
- Check preload.js exists: `ls dist/electron/preload.js`
- Verify Electron version: `npx electron --version`
- Check BrowserWindow configuration

## Stream Issues
- Verify incoming URL format
- Check stream targets configuration
- Review ffmpeg.log for errors
- Test network connectivity

Provide:
1. Diagnosis of the problem
2. Root cause analysis
3. Step-by-step fix instructions
