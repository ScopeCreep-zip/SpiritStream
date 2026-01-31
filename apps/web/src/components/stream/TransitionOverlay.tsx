/**
 * Transition Overlay
 * Renders visual overlay during scene transitions (fadeToColor, stinger, lumaWipe)
 */
import { useState, useEffect, useRef } from 'react';
import { useTransitionStore } from '@/stores/transitionStore';
import { DEFAULT_FADE_COLOR } from '@/types/scene';

export function TransitionOverlay() {
  const { isTransitioning, currentTransition } = useTransitionStore();
  const [stingerReady, setStingerReady] = useState(false);
  const videoRef = useRef<HTMLVideoElement>(null);

  // Handle stinger video preload and playback
  useEffect(() => {
    if (currentTransition?.type === 'stinger' && currentTransition.stingerFilePath) {
      setStingerReady(false);
      // Reset and prepare video
      if (videoRef.current) {
        videoRef.current.currentTime = 0;
        videoRef.current.load();
      }
    }
  }, [currentTransition]);

  // Play stinger video when transition starts
  useEffect(() => {
    if (isTransitioning && currentTransition?.type === 'stinger' && stingerReady && videoRef.current) {
      videoRef.current.currentTime = 0;
      videoRef.current.play().catch((err) => {
        console.error('[TransitionOverlay] Failed to play stinger:', err);
      });
    }
  }, [isTransitioning, currentTransition, stingerReady]);

  // Only render for supported transition types
  if (!isTransitioning || !currentTransition) {
    return null;
  }

  const duration = currentTransition.durationMs;

  // Fade to Color transition
  if (currentTransition.type === 'fadeToColor') {
    const color = currentTransition.color || DEFAULT_FADE_COLOR;
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

  // Stinger transition (video overlay)
  if (currentTransition.type === 'stinger' && currentTransition.stingerFilePath) {
    return (
      <div className="fixed inset-0 pointer-events-none z-[9999]">
        <video
          ref={videoRef}
          src={currentTransition.stingerFilePath}
          className="w-full h-full object-cover"
          muted={currentTransition.stingerAudioMuted ?? true}
          playsInline
          onCanPlayThrough={() => setStingerReady(true)}
          onEnded={() => {
            // Video ended - transition should already be complete by duration
          }}
        />
      </div>
    );
  }

  // Luma Wipe transition (gradient mask)
  if (currentTransition.type === 'lumaWipe') {
    const lumaImage = currentTransition.lumaWipeImage || '/transitions/default-luma.png';
    const invert = currentTransition.lumaWipeInvert ? 'invert(1)' : '';

    return (
      <div
        className="fixed inset-0 pointer-events-none z-[9999] luma-wipe-overlay"
        style={{
          '--transition-duration': `${duration}ms`,
          '--luma-image': `url(${lumaImage})`,
          filter: invert,
        } as React.CSSProperties}
      />
    );
  }

  return null;
}
