/**
 * Media Types Utilities
 *
 * Centralized file type detection and source classification functions.
 * Used to determine which sources need WebRTC streaming vs client-side rendering.
 */

// File extension constants for static media that don't need WebRTC streaming
export const IMAGE_EXTENSIONS = ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'svg'] as const;
export const HTML_EXTENSIONS = ['html', 'htm'] as const;
export const STATIC_EXTENSIONS = [...IMAGE_EXTENSIONS, ...HTML_EXTENSIONS] as const;

// Source types that are rendered client-side (CSS/iframe) and don't need WebRTC
export const CLIENT_RENDERED_SOURCE_TYPES = ['color', 'text', 'browser'] as const;

/**
 * Check if a file path is a static media file (image or HTML)
 */
export function isStaticMediaFile(filePath: string): boolean {
  const ext = filePath.split('.').pop()?.toLowerCase() ?? '';
  return (STATIC_EXTENSIONS as readonly string[]).includes(ext);
}

/**
 * Check if a file path is an image file
 */
export function isImageFile(filePath: string): boolean {
  const ext = filePath.split('.').pop()?.toLowerCase() ?? '';
  return (IMAGE_EXTENSIONS as readonly string[]).includes(ext);
}

/**
 * Check if a file path is an HTML file
 */
export function isHtmlFile(filePath: string): boolean {
  const ext = filePath.split('.').pop()?.toLowerCase() ?? '';
  return (HTML_EXTENSIONS as readonly string[]).includes(ext);
}

/**
 * Check if a source type is client-rendered (CSS/iframe)
 */
export function isClientRenderedSource(sourceType: string): boolean {
  return (CLIENT_RENDERED_SOURCE_TYPES as readonly string[]).includes(sourceType);
}

/**
 * Check if a source requires WebRTC streaming
 */
export function sourceNeedsWebRTC(source: {
  type: string;
  filePath?: string;
}): boolean {
  // Audio devices don't have video to stream
  if (source.type === 'audioDevice') {
    return false;
  }

  // Client-rendered sources (color, text, browser) don't need WebRTC
  if (isClientRenderedSource(source.type)) {
    return false;
  }

  // Static media files (images, HTML) don't need WebRTC
  if (source.type === 'mediaFile' && source.filePath && isStaticMediaFile(source.filePath)) {
    return false;
  }

  // All other sources need WebRTC for live preview
  return true;
}
