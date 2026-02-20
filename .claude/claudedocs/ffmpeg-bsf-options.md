# FFmpeg BSF Options (Deferred Decision)

Date: 2026-02-20  
Status: Deferred until after diagnostics and smoke-test work.

## Context
The current `ffmpeg-libs` pipeline compiles and runs with BSF behavior intentionally disabled because the current `ffmpeg-sys-next` bindings in this branch do not expose the `AVBSFContext` / `av_bsf_*` API shape needed by the existing implementation.

We need to choose a long-term strategy for bitstream-filter handling (`aac_adtstoasc`, `dump_extra`) after diagnostics + hardware-agnostic test scaffolding are completed.

## Options

### 1) Keep BSF disabled (current fallback)
- Pros: fastest path, no extra build/dependency complexity.
- Pros: avoids forking or patching crates.
- Cons: potential compatibility regressions in some ingest/container paths.
- Cons: passthrough/transcode edge cases may behave differently than prior CLI behavior.

### 2) Patch/fork `ffmpeg-sys-next` to expose BSF APIs
- Pros: strongest parity path with FFmpeg C API.
- Pros: keeps logic in Rust once bindings are available.
- Cons: ongoing maintenance burden for fork/patch rebase.
- Cons: version upgrades become more expensive.

### 3) Add a small C shim for BSF operations and call from Rust
- Pros: avoids maintaining a full crate fork.
- Pros: C shim can directly access fields/API patterns that are awkward in current bindings.
- Cons: adds cross-platform C build/toolchain surface.
- Cons: introduces Rust/C boundary complexity.

### 4) Replace BSF intent with encoder/muxer options where possible
- Pros: no BSF API dependency.
- Pros: simple build path.
- Cons: not full parity; may fail for some streams/platforms.
- Cons: likely still leaves edge cases unsolved.

## Working Recommendation
- Near-term: continue with option 1 while finishing diagnostics + smoke tests.
- Next decision point: choose option 2 or 3 based on maintenance preference:
  - prefer dependency purity and in-Rust control -> option 2
  - prefer less crate-fork maintenance -> option 3

## Follow-up Trigger
Revisit this decision immediately after:
1. Runtime diagnostics command is in place.
2. Hardware-agnostic ffmpeg-libs smoke tests are in place.
