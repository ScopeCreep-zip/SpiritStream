/**
 * Shared drag session utilities
 * Extracted from SceneCanvas and UnifiedChannelStrip RAF-throttled
 * mousedown→mousemove→mouseup patterns
 */

/**
 * Creates a RAF-gated mousemove handler.
 * Coalesces rapid mouse events and processes only once per animation frame.
 * Returns a cleanup function that cancels any pending RAF.
 */
export function createRAFThrottle(
  onMove: (e: MouseEvent) => void
): { handler: (e: MouseEvent) => void; cleanup: () => void } {
  let rafPending = false;
  let lastEvent: MouseEvent | null = null;
  let rafId = 0;

  const handler = (e: MouseEvent) => {
    lastEvent = e;
    if (rafPending) return;
    rafPending = true;
    rafId = requestAnimationFrame(() => {
      if (lastEvent) {
        onMove(lastEvent);
      }
      rafPending = false;
    });
  };

  const cleanup = () => {
    if (rafId) cancelAnimationFrame(rafId);
    lastEvent = null;
  };

  return { handler, cleanup };
}

interface DragSessionOptions {
  /** Called on each mousemove (RAF-throttled) */
  onMove: (e: MouseEvent) => void;
  /** Called on mouseup */
  onEnd: (e: MouseEvent) => void;
}

/**
 * Creates a mousedown→move→up drag lifecycle with RAF throttling.
 * Attaches listeners to `window` for capture outside the element.
 * Returns a cleanup function that removes all listeners.
 */
export function createDragSession(opts: DragSessionOptions): () => void {
  const throttle = createRAFThrottle(opts.onMove);

  const handleMouseUp = (e: MouseEvent) => {
    throttle.cleanup();
    window.removeEventListener('mousemove', throttle.handler);
    window.removeEventListener('mouseup', handleMouseUp);
    opts.onEnd(e);
  };

  window.addEventListener('mousemove', throttle.handler);
  window.addEventListener('mouseup', handleMouseUp);

  return () => {
    throttle.cleanup();
    window.removeEventListener('mousemove', throttle.handler);
    window.removeEventListener('mouseup', handleMouseUp);
  };
}
