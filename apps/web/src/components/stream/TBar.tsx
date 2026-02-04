/**
 * T-Bar Component
 * Manual transition fader for Studio Mode
 * Drag from bottom (0%) to top (100%) to manually blend between Preview and Program
 */
import { useRef, useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { useStudioStore } from '@/stores/studioStore';

interface TBarProps {
  disabled?: boolean;
}

export function TBar({ disabled }: TBarProps) {
  const { t } = useTranslation();
  const {
    tBarProgress,
    tBarDragging,
    setTBarProgress,
    startTBarDrag,
    endTBarDrag,
    previewSceneId,
    programSceneId,
  } = useStudioStore();

  const trackRef = useRef<HTMLDivElement>(null);

  // Can't use T-bar if preview and program are the same
  const canUse = previewSceneId !== programSceneId && !disabled;

  // Calculate position from mouse/touch event
  const getProgressFromEvent = useCallback(
    (clientY: number): number => {
      if (!trackRef.current) return 0;
      const rect = trackRef.current.getBoundingClientRect();
      // Invert: top = 1 (full transition), bottom = 0 (no transition)
      const y = clientY - rect.top;
      const progress = 1 - y / rect.height;
      return Math.max(0, Math.min(1, progress));
    },
    []
  );

  // Mouse event handlers
  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      if (!canUse) return;
      e.preventDefault();
      startTBarDrag();
      setTBarProgress(getProgressFromEvent(e.clientY));
    },
    [canUse, startTBarDrag, setTBarProgress, getProgressFromEvent]
  );

  // Global mouse events for drag with RAF throttling
  useEffect(() => {
    if (!tBarDragging) return;

    let rafPending = false;
    let lastY = 0;

    const handleMouseMove = (e: MouseEvent) => {
      lastY = e.clientY;
      if (rafPending) return;
      rafPending = true;
      requestAnimationFrame(() => {
        setTBarProgress(getProgressFromEvent(lastY));
        rafPending = false;
      });
    };

    const handleMouseUp = () => {
      endTBarDrag();
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [tBarDragging, setTBarProgress, endTBarDrag, getProgressFromEvent]);

  // Touch event handlers for mobile
  const handleTouchStart = useCallback(
    (e: React.TouchEvent) => {
      if (!canUse) return;
      e.preventDefault();
      startTBarDrag();
      setTBarProgress(getProgressFromEvent(e.touches[0].clientY));
    },
    [canUse, startTBarDrag, setTBarProgress, getProgressFromEvent]
  );

  // Touch events with RAF throttling
  useEffect(() => {
    if (!tBarDragging) return;

    let rafPending = false;
    let lastY = 0;

    const handleTouchMove = (e: TouchEvent) => {
      lastY = e.touches[0].clientY;
      if (rafPending) return;
      rafPending = true;
      requestAnimationFrame(() => {
        setTBarProgress(getProgressFromEvent(lastY));
        rafPending = false;
      });
    };

    const handleTouchEnd = () => {
      endTBarDrag();
    };

    document.addEventListener('touchmove', handleTouchMove, { passive: false });
    document.addEventListener('touchend', handleTouchEnd);
    document.addEventListener('touchcancel', handleTouchEnd);

    return () => {
      document.removeEventListener('touchmove', handleTouchMove);
      document.removeEventListener('touchend', handleTouchEnd);
      document.removeEventListener('touchcancel', handleTouchEnd);
    };
  }, [tBarDragging, setTBarProgress, endTBarDrag, getProgressFromEvent]);

  const progressPercent = Math.round(tBarProgress * 100);

  return (
    <div className="flex flex-col items-center gap-1">
      <span className="text-[10px] text-[var(--text-muted)] uppercase tracking-wide">
        {t('stream.tBar', { defaultValue: 'T-Bar' })}
      </span>

      {/* T-bar track */}
      <div
        ref={trackRef}
        className={`relative w-4 h-32 rounded-full transition-colors ${
          canUse
            ? 'bg-[var(--bg-sunken)] cursor-ns-resize'
            : 'bg-[var(--bg-sunken)] opacity-50 cursor-not-allowed'
        }`}
        onMouseDown={handleMouseDown}
        onTouchStart={handleTouchStart}
      >
        {/* Progress fill */}
        <div
          className={`absolute bottom-0 left-0 right-0 rounded-full transition-colors ${
            tBarDragging ? 'bg-primary' : 'bg-primary/60'
          }`}
          style={{ height: `${tBarProgress * 100}%` }}
        />

        {/* Center line marker (50% threshold) */}
        <div className="absolute left-0 right-0 top-1/2 -translate-y-1/2 h-0.5 bg-[var(--border-strong)]" />

        {/* Thumb handle */}
        <div
          className={`absolute left-1/2 -translate-x-1/2 w-6 h-4 rounded transition-all ${
            tBarDragging
              ? 'bg-primary border-2 border-primary-foreground shadow-lg scale-110'
              : canUse
                ? 'bg-[var(--bg-elevated)] border-2 border-[var(--border-default)] shadow-md hover:border-primary'
                : 'bg-[var(--bg-elevated)] border-2 border-[var(--border-default)] opacity-50'
          }`}
          style={{ bottom: `calc(${tBarProgress * 100}% - 8px)` }}
        >
          {/* Grip lines */}
          <div className="flex flex-col items-center justify-center h-full gap-0.5">
            <div className="w-2 h-0.5 bg-[var(--text-muted)] rounded-full" />
            <div className="w-2 h-0.5 bg-[var(--text-muted)] rounded-full" />
          </div>
        </div>
      </div>

      {/* Progress indicator */}
      <span
        className={`text-[10px] tabular-nums ${
          tBarDragging ? 'text-primary font-medium' : 'text-[var(--text-muted)]'
        }`}
      >
        {progressPercent}%
      </span>
    </div>
  );
}
