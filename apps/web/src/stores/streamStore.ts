/// Re-export shim — the stream store now lives in `./stream/`. Existing
/// imports from `@/stores/streamStore` continue to resolve through this
/// file; new code should import from `@/stores/stream` directly.
export { useStreamStore } from './stream';
export type { FFmpegStats, GroupStats, StreamState } from './stream';
