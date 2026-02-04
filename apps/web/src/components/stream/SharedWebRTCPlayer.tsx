/**
 * SharedWebRTCPlayer Component
 * Video player that uses persistent WebRTC connections via go2rtc
 *
 * Uses the persistent WebRTC connection store that keeps connections alive
 * regardless of page visibility or navigation. Connections are managed at
 * the app level by WebRTCConnectionManager.
 *
 * Features:
 * - Skeleton loading with shimmer animation for polished loading UX
 * - Smooth fade-in transition when video becomes ready
 * - Persistent connections survive page visibility changes
 * - Connections stay alive during navigation between views
 */

import { useRef, useEffect, useState } from 'react';
import { useWebRTCStream } from '@/hooks/useWebRTCStream';
import { Radio, Film, Monitor, Camera, Usb, Mic } from 'lucide-react';
import type { Source } from '@/types/profile';

interface SharedWebRTCPlayerProps {
  sourceId: string;
  sourceName?: string;
  sourceType?: Source['type'];
  width: number;
  height: number;
  className?: string;
}

/** Icon component for different source types */
function SourceTypeIcon({ type, className = 'w-6 h-6' }: { type?: Source['type']; className?: string }) {
  switch (type) {
    case 'rtmp':
      return <Radio className={className} />;
    case 'mediaFile':
      return <Film className={className} />;
    case 'screenCapture':
      return <Monitor className={className} />;
    case 'camera':
      return <Camera className={className} />;
    case 'captureCard':
      return <Usb className={className} />;
    case 'audioDevice':
      return <Mic className={className} />;
    default:
      return <Monitor className={className} />;
  }
}

export function SharedWebRTCPlayer({
  sourceId,
  sourceName = 'Source',
  sourceType,
  width,
  height,
  className = '',
}: SharedWebRTCPlayerProps) {
  const { status, stream, error, retry } = useWebRTCStream(sourceId);
  const videoRef = useRef<HTMLVideoElement>(null);
  const [videoReady, setVideoReady] = useState(false);
  const mountedRef = useRef(true);

  // Track mounted state for cleanup - prevents state updates after unmount
  useEffect(() => {
    mountedRef.current = true;
    return () => { mountedRef.current = false; };
  }, []);

  // Attach stream to video element when it changes
  // Apply low-latency hints for broadcast-grade preview
  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;

    if (stream) {
      video.srcObject = stream;
      setVideoReady(false); // Reset when stream changes - wait for decoder to initialize

      // Low-latency hints for live content
      video.playsInline = true;
      video.muted = true;
      video.disablePictureInPicture = true;
      // Disable audio pitch preservation to avoid stretching artifacts
      (video as HTMLVideoElement & { preservesPitch?: boolean }).preservesPitch = false;

      // Catch up to live edge on play - reduces latency from buffered content
      const handleCanPlay = () => {
        if (video.buffered.length > 0) {
          const liveEdge = video.buffered.end(video.buffered.length - 1);
          // Only jump if we're more than 100ms behind live edge
          if (liveEdge - video.currentTime > 0.1) {
            video.currentTime = liveEdge;
          }
        }
      };
      video.addEventListener('canplay', handleCanPlay, { once: true });

      return () => {
        video.removeEventListener('canplay', handleCanPlay);
      };
    } else {
      video.srcObject = null;
      setVideoReady(false);
    }
  }, [stream]);

  // Listen for video ready events - event-driven approach (no polling)
  // The green tint appears when H.264 decoder hasn't received a keyframe yet
  // We wait for BOTH dimensions AND readyState >= HAVE_CURRENT_DATA
  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;

    // Check if video has actually decoded content
    // readyState >= 2 (HAVE_CURRENT_DATA) means decoder has rendered at least one frame
    const checkVideoReady = () => {
      if (!mountedRef.current) return;
      if (video.videoWidth > 0 &&
          video.videoHeight > 0 &&
          video.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA) {
        setVideoReady(true);
      }
    };

    // Handle standard video events
    const handleVideoEvent = () => {
      checkVideoReady();
    };

    // Handle resize event - fired when video dimensions change (after decoder initialization)
    // This is the reliable signal that the decoder has received a keyframe
    const handleResize = () => {
      checkVideoReady();
    };

    // Listen to multiple events for better coverage across stream types
    // - loadeddata: metadata and first frame loaded
    // - canplay: enough data to start playback
    // - resize: dimensions changed (fires after decoder has a keyframe)
    // - playing: playback actually started
    video.addEventListener('loadeddata', handleVideoEvent);
    video.addEventListener('canplay', handleVideoEvent);
    video.addEventListener('resize', handleResize);
    video.addEventListener('playing', handleVideoEvent);

    // Check immediately in case video is already ready
    checkVideoReady();

    return () => {
      video.removeEventListener('loadeddata', handleVideoEvent);
      video.removeEventListener('canplay', handleVideoEvent);
      video.removeEventListener('resize', handleResize);
      video.removeEventListener('playing', handleVideoEvent);
    };
  }, [stream]);

  return (
    <div
      className={`relative bg-[var(--bg-sunken)] overflow-hidden ${className}`}
      style={{ width, height }}
    >
      {/* Video element for WebRTC/MSE - with smooth fade-in transition */}
      {/* Only show when BOTH status is playing AND video has decoded first frame (videoReady) */}
      {/* This prevents the green tint that appears before the H.264 decoder receives a keyframe */}
      <video
        ref={videoRef}
        autoPlay
        muted
        playsInline
        className={`w-full h-full object-cover transition-opacity duration-300 ${
          status === 'playing' && videoReady ? 'opacity-100' : 'opacity-0'
        }`}
      />

      {/* Skeleton loading state - shows until video is actually ready to display */}
      {/* This covers: idle, loading, connecting, AND playing-but-not-yet-decoded states */}
      {(status === 'idle' || status === 'loading' || status === 'connecting' || (status === 'playing' && !videoReady)) && (
        <div className="absolute inset-0 bg-[var(--bg-sunken)] overflow-hidden">
          {/* Animated shimmer effect */}
          <div className="absolute inset-0 bg-gradient-to-r from-transparent via-[var(--bg-elevated)]/50 to-transparent skeleton-shimmer" />
          {/* Source indicator */}
          <div className="absolute inset-0 flex flex-col items-center justify-center gap-2">
            <div className="w-10 h-10 rounded-full bg-[var(--bg-elevated)] flex items-center justify-center">
              <SourceTypeIcon type={sourceType} className="w-5 h-5 text-[var(--text-muted)]" />
            </div>
            <span className="text-xs text-[var(--text-muted)] text-center px-2 truncate max-w-full">
              {sourceName}
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

      {/* Shared WebRTC indicator when playing */}
      {status === 'playing' && (
        <div className="absolute top-1 right-1 px-1.5 py-0.5 text-[10px] font-medium bg-[var(--status-live)]/80 text-white rounded">
          WebRTC
        </div>
      )}
    </div>
  );
}
