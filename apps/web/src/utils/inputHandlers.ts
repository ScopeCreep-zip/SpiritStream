/**
 * Shared input event handlers
 * Extracted from inline patterns across PropertiesPanel, SceneBar,
 * AdvancedAudioProperties, BrowserSourceEditor, AudioFilterChainEditor
 */
import type { KeyboardEvent } from 'react';

/** Blur the target element when Enter is pressed */
export function blurOnEnter(e: KeyboardEvent<HTMLInputElement | HTMLTextAreaElement>): void {
  if (e.key === 'Enter') {
    e.currentTarget.blur();
  }
}

/**
 * Wraps a callback with e.stopPropagation().
 * Returns a new handler that stops event bubbling before calling the original.
 */
export function stopPropagationAnd<E extends { stopPropagation(): void }>(
  fn: (e: E) => void
): (e: E) => void {
  return (e: E) => {
    e.stopPropagation();
    fn(e);
  };
}

/**
 * Returns an onKeyDown handler that:
 * - Blurs on Enter (submit)
 * - Calls onCancel and blurs on Escape
 */
export function escapeToCancel(
  onCancel: () => void
): (e: KeyboardEvent<HTMLInputElement>) => void {
  return (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.currentTarget.blur();
    } else if (e.key === 'Escape') {
      onCancel();
      e.currentTarget.blur();
    }
  };
}
