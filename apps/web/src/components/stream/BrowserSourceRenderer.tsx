/**
 * Browser Source Renderer
 * Renders web pages via iframe with scaling support
 */
import { useRef, useState, useEffect } from 'react';
import { Globe, RefreshCw } from 'lucide-react';
import type { BrowserSource } from '@/types/source';

interface BrowserSourceRendererProps {
  source: BrowserSource;
  width: number;
  height: number;
}

export function BrowserSourceRenderer({ source, width, height }: BrowserSourceRendererProps) {
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const [error, setError] = useState(false);
  const [loading, setLoading] = useState(true);
  const [key, setKey] = useState(0);

  // Calculate scale to fit the layer dimensions
  const scaleX = width / source.width;
  const scaleY = height / source.height;
  const scale = Math.min(scaleX, scaleY);

  // Auto-refresh support
  useEffect(() => {
    if (!source.refreshInterval || source.refreshInterval <= 0) return;

    const interval = setInterval(() => {
      setKey((k) => k + 1);
      setLoading(true);
      setError(false);
    }, source.refreshInterval * 1000);

    return () => clearInterval(interval);
  }, [source.refreshInterval]);

  // Reset error state when URL or refreshToken changes
  useEffect(() => {
    setError(false);
    setLoading(true);
    setKey((k) => k + 1);
  }, [source.url, source.refreshToken]);

  const handleLoad = () => {
    setLoading(false);
  };

  const handleError = () => {
    setError(true);
    setLoading(false);
  };

  const handleRefresh = () => {
    setKey((k) => k + 1);
    setLoading(true);
    setError(false);
  };

  // Validate URL
  const isValidUrl = (() => {
    if (!source.url) return false;
    try {
      const parsed = new URL(source.url);
      return ['http:', 'https:'].includes(parsed.protocol);
    } catch {
      return false;
    }
  })();

  if (!isValidUrl) {
    return (
      <div
        className="flex items-center justify-center bg-[var(--bg-sunken)]"
        style={{ width, height }}
      >
        <div className="text-center text-[var(--text-muted)]">
          <Globe className="w-8 h-8 mx-auto mb-2 opacity-50" />
          <p className="text-sm">Enter a valid URL</p>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div
        className="flex items-center justify-center bg-[var(--bg-sunken)]"
        style={{ width, height }}
      >
        <div className="text-center text-[var(--text-muted)]">
          <Globe className="w-8 h-8 mx-auto mb-2 opacity-50" />
          <p className="text-sm">Cannot load this URL</p>
          <p className="text-xs opacity-60 mb-2">Site may block embedding</p>
          <button
            onClick={handleRefresh}
            className="inline-flex items-center gap-1 px-2 py-1 text-xs bg-[var(--bg-elevated)] rounded hover:bg-[var(--bg-hover)] transition-colors"
          >
            <RefreshCw className="w-3 h-3" />
            Retry
          </button>
        </div>
      </div>
    );
  }

  return (
    <div
      className="relative overflow-hidden bg-[var(--bg-sunken)]"
      style={{ width, height }}
    >
      {loading && (
        <div className="absolute inset-0 flex items-center justify-center bg-[var(--bg-sunken)] z-10">
          <div className="flex flex-col items-center gap-2">
            <div className="w-6 h-6 border-2 border-primary/30 border-t-primary rounded-full animate-spin" />
            <span className="text-[var(--text-muted)] text-xs">Loading...</span>
          </div>
        </div>
      )}
      <iframe
        key={key}
        ref={iframeRef}
        src={source.url}
        style={{
          width: source.width,
          height: source.height,
          border: 'none',
          transform: `scale(${scale})`,
          transformOrigin: 'top left',
        }}
        sandbox="allow-scripts allow-same-origin allow-forms"
        loading="lazy"
        onLoad={handleLoad}
        onError={handleError}
      />
    </div>
  );
}
