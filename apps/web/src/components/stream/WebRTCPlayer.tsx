/**
 * WebRTCPlayer Component
 * Video player that uses WebRTC streaming via go2rtc
 *
 * Features:
 * - WebRTC (WHEP) connection for low latency
 * - MSE over WebSocket as secondary method
 * - No fallback to snapshot polling - WebRTC only
 */

import { useWebRTCPreview } from '@/hooks/useWebRTCPreview';

interface WebRTCPlayerProps {
  sourceId: string;
  sourceName?: string;
  width: number;
  height: number;
  className?: string;
  /** Pass deviceId/displayId to force reconnect when device changes */
  refreshKey?: string;
}

export function WebRTCPlayer({
  sourceId,
  sourceName = 'Source',
  width,
  height,
  className = '',
  refreshKey,
}: WebRTCPlayerProps) {
  const { status, videoRef, error, retry } = useWebRTCPreview(sourceId, refreshKey);

  return (
    <div
      className={`relative bg-[var(--bg-sunken)] overflow-hidden ${className}`}
      style={{ width, height }}
    >
      {/* Video element for WebRTC/MSE */}
      <video
        ref={videoRef}
        autoPlay
        muted
        playsInline
        className="w-full h-full object-cover"
        style={{ display: status === 'playing' ? 'block' : 'none' }}
      />

      {/* Loading state */}
      {(status === 'loading' || status === 'connecting') && (
        <div className="absolute inset-0 flex items-center justify-center bg-gradient-to-br from-[var(--bg-elevated)] to-[var(--bg-sunken)]">
          <div className="flex flex-col items-center gap-2">
            <div className="w-6 h-6 border-2 border-primary/30 border-t-primary rounded-full animate-spin" />
            <span className="text-[var(--text-muted)] text-xs">
              {status === 'loading' ? 'Initializing WebRTC...' : 'Connecting...'}
            </span>
          </div>
        </div>
      )}

      {/* Error/Unavailable state */}
      {(status === 'error' || status === 'unavailable') && (
        <div className="absolute inset-0 flex flex-col items-center justify-center bg-gradient-to-br from-[var(--bg-elevated)] to-[var(--bg-sunken)] gap-2">
          <svg
            xmlns="http://www.w3.org/2000/svg"
            className="w-6 h-6 text-[var(--status-error)]"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <circle cx="12" cy="12" r="10" />
            <line x1="12" y1="8" x2="12" y2="12" />
            <line x1="12" y1="16" x2="12.01" y2="16" />
          </svg>
          <span className="text-[var(--text-muted)] text-xs text-center px-2">
            {status === 'unavailable'
              ? 'WebRTC server unavailable'
              : error || 'Connection failed'}
          </span>
          <span className="text-[var(--text-muted)] text-[10px] opacity-60">
            {sourceName}
          </span>
          <button
            type="button"
            onClick={retry}
            className="mt-1 px-3 py-1 text-xs bg-[var(--bg-elevated)] rounded-md hover:bg-[var(--bg-base)] transition-colors"
          >
            Retry
          </button>
        </div>
      )}

      {/* WebRTC indicator when playing */}
      {status === 'playing' && (
        <div className="absolute top-1 right-1 px-1.5 py-0.5 text-[10px] font-medium bg-[var(--status-live)]/80 text-white rounded">
          WebRTC
        </div>
      )}
    </div>
  );
}
