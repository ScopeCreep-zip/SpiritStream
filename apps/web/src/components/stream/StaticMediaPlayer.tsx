/**
 * StaticMediaPlayer Component
 * Displays static media files (images, HTML) that don't need WebRTC streaming
 *
 * For images: Renders an <img> tag with object-cover styling
 * For HTML: Renders an <iframe> pointing to the local file
 */

import { useState } from 'react';
import { Image, FileCode } from 'lucide-react';
import { api } from '@/lib/backend';

interface StaticMediaPlayerProps {
  filePath: string;
  isImage: boolean;
  width: number;
  height: number;
  sourceName?: string;
  className?: string;
  /** Native resolution for HTML content (default: 1920x1080) */
  nativeWidth?: number;
  nativeHeight?: number;
}

export function StaticMediaPlayer({
  filePath,
  isImage,
  width,
  height,
  sourceName = 'Media',
  className = '',
  nativeWidth = 1920,
  nativeHeight = 1080,
}: StaticMediaPlayerProps) {
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);

  // Get the file URL from the backend API
  const fileUrl = api.preview.getStaticFileUrl(filePath);

  // Calculate scale factors for HTML content to fill exact layer dimensions
  const scaleX = width / nativeWidth;
  const scaleY = height / nativeHeight;

  const handleLoad = () => {
    setLoading(false);
    setError(false);
  };

  const handleError = () => {
    setLoading(false);
    setError(true);
  };

  return (
    <div
      className={`relative bg-[var(--bg-sunken)] overflow-hidden ${className}`}
      style={{ width, height }}
    >
      {isImage ? (
        <img
          src={fileUrl}
          alt={sourceName}
          className={`w-full h-full object-cover transition-opacity duration-300 ${
            loading ? 'opacity-0' : 'opacity-100'
          }`}
          onLoad={handleLoad}
          onError={handleError}
        />
      ) : (
        // HTML content: render at native resolution and scale to fill layer exactly
        // Using scale(scaleX, scaleY) allows independent X/Y scaling to match layer dimensions
        <iframe
          src={fileUrl}
          title={sourceName}
          className={`absolute border-0 transition-opacity duration-300 ${
            loading ? 'opacity-0' : 'opacity-100'
          }`}
          style={{
            width: nativeWidth,
            height: nativeHeight,
            transform: `scale(${scaleX}, ${scaleY})`,
            transformOrigin: 'top left',
          }}
          onLoad={handleLoad}
          onError={handleError}
          sandbox="allow-scripts allow-same-origin"
          // credentialless allows cross-origin content when COEP: require-corp is set
          // @ts-expect-error - credentialless is a valid HTML attribute but not in React types yet
          credentialless=""
        />
      )}

      {/* Loading skeleton */}
      {loading && !error && (
        <div className="absolute inset-0 bg-[var(--bg-sunken)] overflow-hidden">
          <div className="absolute inset-0 bg-gradient-to-r from-transparent via-[var(--bg-elevated)]/50 to-transparent skeleton-shimmer" />
          <div className="absolute inset-0 flex flex-col items-center justify-center gap-2">
            <div className="w-10 h-10 rounded-full bg-[var(--bg-elevated)] flex items-center justify-center">
              {isImage ? (
                <Image className="w-5 h-5 text-[var(--text-muted)]" />
              ) : (
                <FileCode className="w-5 h-5 text-[var(--text-muted)]" />
              )}
            </div>
            <span className="text-xs text-[var(--text-muted)] text-center px-2 truncate max-w-full">
              {sourceName}
            </span>
          </div>
        </div>
      )}

      {/* Error state */}
      {error && (
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
            Failed to load {isImage ? 'image' : 'HTML'}
          </span>
          <span className="text-[var(--text-muted)] text-[10px] opacity-60 max-w-full truncate px-4">
            {sourceName}
          </span>
        </div>
      )}

      {/* Type indicator */}
      {!loading && !error && (
        <div className="absolute top-1 right-1 px-1.5 py-0.5 text-[10px] font-medium bg-[var(--bg-elevated)]/80 text-[var(--text-secondary)] rounded">
          {isImage ? 'IMG' : 'HTML'}
        </div>
      )}
    </div>
  );
}
