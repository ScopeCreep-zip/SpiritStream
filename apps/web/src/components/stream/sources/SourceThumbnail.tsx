import { memo, useRef, useState, useEffect } from 'react';
import { Mic } from 'lucide-react';
import { useWebRTCStream } from '@/hooks/useWebRTCStream';
import { isStaticMediaFile, isImageFile, isClientRenderedSource } from '@/lib/mediaTypes';
import { api } from '@/lib/backend';
import { SourceIcon } from './SourceIcon';
import type { Source } from '@/types/profile';

interface SourceThumbnailProps {
  sourceId: string;
  sourceType: Source['type'];
  /** For mediaFile sources, the file path to check if it's a static image/HTML */
  filePath?: string;
}

/**
 * Live thumbnail preview for a source using WebRTC
 * Uses persistent WebRTC connections managed by WebRTCConnectionManager
 * Connections stay alive regardless of visibility to prevent reconnection delays
 * Memoized to prevent unnecessary re-renders when sibling components update
 */
export const SourceThumbnail = memo(function SourceThumbnail({
  sourceId,
  sourceType,
  filePath,
}: SourceThumbnailProps) {
  // Check if this is a static media file (image/HTML) that doesn't need WebRTC
  const isStatic = filePath && isStaticMediaFile(filePath);
  const isImage = filePath && isImageFile(filePath);
  // Check if this is a client-rendered source (color, text, browser) that doesn't need WebRTC
  const isClientRendered = isClientRenderedSource(sourceType);

  // Get WebRTC stream from persistent connection store
  // Connection is managed by WebRTCConnectionManager, not this component
  // Skip WebRTC for static media files and client-rendered sources
  const { status, stream, retry } = useWebRTCStream(isStatic || isClientRendered ? '' : sourceId);
  const videoRef = useRef<HTMLVideoElement>(null);
  const [videoReady, setVideoReady] = useState(false);
  const mountedRef = useRef(true);

  // Track mounted state for cleanup - prevents state updates after unmount
  useEffect(() => {
    mountedRef.current = true;
    return () => { mountedRef.current = false; };
  }, []);

  // Attach stream to video element when it changes
  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    video.srcObject = stream;
    setVideoReady(false); // Reset when stream changes - wait for decoder to initialize
  }, [stream]);

  // Listen for video ready events - using multiple signals for reliability
  // The green tint appears when H.264 decoder hasn't received a keyframe yet
  // We wait for BOTH dimensions AND readyState >= HAVE_CURRENT_DATA
  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;

    let frameCheckId: number | null = null;
    let isChecking = false; // Prevent overlapping RAF loops from concurrent events

    // Check if video has actually decoded content
    // readyState >= 2 (HAVE_CURRENT_DATA) means decoder has rendered at least one frame
    // This is more reliable than just checking dimensions, which can be set from H.264 SPS
    // metadata before actual pixels are decoded
    const checkVideoReady = () => {
      if (!mountedRef.current) return true; // Stop if unmounted
      if (video.videoWidth > 0 &&
          video.videoHeight > 0 &&
          video.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA) {
        setVideoReady(true);
        return true;
      }
      return false;
    };

    // Handle loadeddata/canplay events - but also verify decoder readiness
    const handleVideoEvent = () => {
      if (isChecking) return; // Prevent concurrent polling loops
      if (checkVideoReady()) return;

      isChecking = true;
      let attempts = 0;
      // Poll for up to ~1000ms to cover screen capture's 500ms keyframe interval
      // (15-frame keyframe interval @ 30fps = ~500ms for first keyframe)
      const MAX_POLL_ATTEMPTS = 60;

      const pollDimensions = () => {
        if (!mountedRef.current || checkVideoReady() || attempts++ >= MAX_POLL_ATTEMPTS) {
          frameCheckId = null;
          isChecking = false;
          return;
        }
        frameCheckId = requestAnimationFrame(pollDimensions);
      };
      frameCheckId = requestAnimationFrame(pollDimensions);
    };

    // Listen to multiple events for better coverage across stream types
    video.addEventListener('loadeddata', handleVideoEvent);
    video.addEventListener('canplay', handleVideoEvent);

    // Check immediately in case video is already ready
    handleVideoEvent();

    return () => {
      video.removeEventListener('loadeddata', handleVideoEvent);
      video.removeEventListener('canplay', handleVideoEvent);
      if (frameCheckId !== null) {
        cancelAnimationFrame(frameCheckId);
      }
    };
  }, [stream]);

  // Only show preview for video sources
  const hasVideo = sourceType !== 'audioDevice';

  if (!hasVideo) {
    // Audio-only placeholder
    return (
      <div className="w-16 h-9 bg-[var(--bg-sunken)] rounded flex items-center justify-center flex-shrink-0">
        <Mic className="w-4 h-4 text-muted" />
      </div>
    );
  }

  // Static media file (image/HTML) - render directly without WebRTC
  if (isStatic && filePath) {
    const fileUrl = api.preview.getStaticFileUrl(filePath);
    return (
      <div className="relative w-16 h-9 bg-[var(--bg-sunken)] rounded overflow-hidden flex-shrink-0">
        {isImage ? (
          <img
            src={fileUrl}
            alt=""
            className="w-full h-full object-cover"
            onError={(e) => {
              // Hide broken image and show fallback
              e.currentTarget.style.display = 'none';
            }}
          />
        ) : (
          // HTML file - just show an icon
          <div className="w-full h-full flex items-center justify-center">
            <SourceIcon type={sourceType} />
          </div>
        )}
      </div>
    );
  }

  // Client-rendered sources (color, text, browser) - show placeholder with icon
  if (isClientRendered) {
    return (
      <div className="relative w-16 h-9 bg-[var(--bg-sunken)] rounded overflow-hidden flex-shrink-0">
        <div className="w-full h-full flex items-center justify-center">
          <SourceIcon type={sourceType} />
        </div>
      </div>
    );
  }

  return (
    <div className="relative w-16 h-9 bg-[var(--bg-sunken)] rounded overflow-hidden flex-shrink-0">
      {/* Video element for WebRTC - with smooth fade-in transition */}
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
          {/* Source type icon */}
          <div className="absolute inset-0 flex items-center justify-center">
            <SourceIcon type={sourceType} />
          </div>
        </div>
      )}

      {/* Error/Unavailable state - show icon with retry on click */}
      {(status === 'error' || status === 'unavailable') && (
        <div
          className="absolute inset-0 flex items-center justify-center cursor-pointer hover:bg-muted/20 transition-colors"
          onClick={(e) => {
            e.stopPropagation();
            retry();
          }}
          title="Click to retry preview"
        >
          <SourceIcon type={sourceType} />
        </div>
      )}
    </div>
  );
});
