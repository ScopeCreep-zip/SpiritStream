/// File-browser platform helpers — OS detection, path arithmetic, and
/// the server-error → user-facing-string map. Pure logic, no React.

export type TFunction = (key: string, defaultValue: string) => string;
export type Platform = 'windows' | 'macos' | 'linux' | 'unknown';
export type QuickPath = { label: string; path: string };

export function detectPlatform(): Platform {
  if (typeof navigator === 'undefined') {
    return 'unknown';
  }

  const nav = navigator as Navigator & { userAgentData?: { platform?: string } };
  const platformHint = nav.userAgentData?.platform || nav.platform || nav.userAgent || '';

  if (/windows/i.test(platformHint)) return 'windows';
  if (/mac/i.test(platformHint)) return 'macos';
  if (/linux/i.test(platformHint)) return 'linux';
  return 'unknown';
}

export function quickPathsFor(platform: Platform): QuickPath[] {
  if (platform === 'windows') {
    return [
      { label: 'Program Files', path: 'C:\\Program Files' },
      { label: 'Program Files (x86)', path: 'C:\\Program Files (x86)' },
    ];
  }
  if (platform === 'macos') {
    return [
      { label: '/usr/local/bin', path: '/usr/local/bin' },
      { label: '/opt/homebrew/bin', path: '/opt/homebrew/bin' },
    ];
  }
  if (platform === 'linux') {
    return [
      { label: '/usr/bin', path: '/usr/bin' },
      { label: '/usr/local/bin', path: '/usr/local/bin' },
      { label: '/opt', path: '/opt' },
    ];
  }
  return [];
}

export function getPathSeparator(path: string): string {
  return path.includes('\\') ? '\\' : '/';
}

export function joinPath(base: string, entry: string): string {
  const separator = getPathSeparator(base);
  if (!base || base.endsWith(separator)) {
    return `${base}${entry}`;
  }
  return `${base}${separator}${entry}`;
}

export function getInitialBrowsePath(path: string): string {
  // End-anchored character-class repetition has linear time complexity
  // — sonarjs's slow-regex heuristic over-fires on the `+$` shape.
  // eslint-disable-next-line sonarjs/slow-regex
  const trimmed = path.replace(/[\\/]+$/, '');
  const lastSlash = trimmed.lastIndexOf('/');
  const lastBackslash = trimmed.lastIndexOf('\\');
  const lastSep = Math.max(lastSlash, lastBackslash);
  if (lastSep > 0) {
    return trimmed.substring(0, lastSep);
  }

  const driveMatch = trimmed.match(/^[A-Za-z]:/);
  if (driveMatch) {
    return `${driveMatch[0]}\\`;
  }

  return '/';
}

export function formatSize(bytes?: number): string {
  if (bytes === undefined) return '';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function getFriendlyError(serverError: string, t: TFunction): string {
  if (serverError.includes('Access to this directory is not allowed')) {
    return t(
      'fileBrowser.accessDenied',
      'This location is outside the allowed browsing area. You can browse your home directory and common system folders.'
    );
  }
  if (serverError.includes('Directory not found')) {
    return t('fileBrowser.directoryNotFound', 'Directory not found.');
  }
  if (serverError.includes('Path is not a directory')) {
    return t('fileBrowser.notADirectory', 'The selected path is not a directory.');
  }
  if (serverError.includes('Failed to read directory')) {
    return t('fileBrowser.readError', 'Unable to read directory contents. Check permissions.');
  }
  return serverError;
}
