/**
 * Transition Overlay
 * Renders visual overlay during scene transitions (especially for fadeToColor)
 */
import { useTransitionStore } from '@/stores/transitionStore';
import { DEFAULT_FADE_COLOR } from '@/types/scene';

export function TransitionOverlay() {
  const { isTransitioning, currentTransition } = useTransitionStore();

  // Only render for fadeToColor transitions
  if (!isTransitioning || !currentTransition || currentTransition.type !== 'fadeToColor') {
    return null;
  }

  const color = currentTransition.color || DEFAULT_FADE_COLOR;
  const duration = currentTransition.durationMs;

  return (
    <div
      className="fixed inset-0 pointer-events-none z-[9999] fade-to-color-overlay"
      style={{
        '--transition-duration': `${duration}ms`,
        '--transition-color': color,
      } as React.CSSProperties}
    />
  );
}
