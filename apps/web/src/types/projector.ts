/**
 * Projector types for multi-output display system
 * Supports OBS-parity projector features: scenes, sources, preview/program, multiview
 */

/**
 * Type of content being projected
 */
export type ProjectorType = 'source' | 'scene' | 'preview' | 'program' | 'multiview';

/**
 * Display mode for the projector
 */
export type ProjectorDisplayMode = 'fullscreen' | 'windowed';

/**
 * Monitor/display information for projector target selection
 */
export interface MonitorInfo {
  /** Unique identifier for the monitor */
  id: string;
  /** Display name (e.g., "Display 1", "DELL U2720Q") */
  name: string;
  /** Monitor width in pixels */
  width: number;
  /** Monitor height in pixels */
  height: number;
  /** X position in virtual screen space */
  x: number;
  /** Y position in virtual screen space */
  y: number;
  /** Whether this is the primary display */
  isPrimary: boolean;
  /** Scale factor (e.g., 2 for Retina) */
  scaleFactor?: number;
}

/**
 * Configuration for a projector instance
 */
export interface ProjectorConfig {
  /** Unique ID for this projector instance */
  id: string;
  /** Type of projector content */
  type: ProjectorType;
  /** Display mode */
  displayMode: ProjectorDisplayMode;
  /** Target source or scene ID (not needed for preview/program/multiview) */
  targetId?: string;
  /** Target monitor ID for fullscreen mode */
  monitorId?: string;
  /** Profile name (required for loading scene data) */
  profileName: string;
  /** Keep window above others */
  alwaysOnTop: boolean;
  /** Hide mouse cursor in projector */
  hideCursor: boolean;
}

/**
 * State of an active projector window
 */
export interface ProjectorInstance {
  /** Configuration for this projector */
  config: ProjectorConfig;
  /** Window reference (browser popup) */
  windowRef?: Window | null;
  /** Whether the projector is currently active */
  isActive: boolean;
}

/**
 * Context menu options for projector actions
 */
export interface ProjectorMenuContext {
  /** Type of projector to open */
  type: ProjectorType;
  /** Target ID (source or scene) */
  targetId?: string;
  /** Target name for display */
  targetName?: string;
  /** Profile name */
  profileName: string;
}

/**
 * Generate a unique projector ID
 */
export function generateProjectorId(type: ProjectorType, targetId?: string): string {
  const base = targetId ? `${type}-${targetId}` : type;
  return `${base}-${Date.now()}`;
}

/**
 * Build URL parameters for projector view
 */
export function buildProjectorUrl(config: Omit<ProjectorConfig, 'id' | 'isActive'>): string {
  const params = new URLSearchParams();
  params.set('type', config.type);
  params.set('profileName', config.profileName);
  if (config.targetId) {
    params.set('targetId', config.targetId);
  }
  if (config.displayMode) {
    params.set('mode', config.displayMode);
  }
  if (config.alwaysOnTop) {
    params.set('alwaysOnTop', '1');
  }
  if (config.hideCursor) {
    params.set('hideCursor', '1');
  }
  return `/projector?${params.toString()}`;
}

/**
 * Parse projector config from URL search params
 */
export function parseProjectorParams(searchParams: URLSearchParams): Partial<ProjectorConfig> {
  return {
    type: (searchParams.get('type') as ProjectorType) || 'scene',
    profileName: searchParams.get('profileName') || '',
    targetId: searchParams.get('targetId') || searchParams.get('sceneId') || undefined,
    displayMode: (searchParams.get('mode') as ProjectorDisplayMode) || 'windowed',
    alwaysOnTop: searchParams.get('alwaysOnTop') === '1',
    hideCursor: searchParams.get('hideCursor') === '1',
  };
}

/**
 * Get display label for projector type
 */
export function getProjectorTypeLabel(type: ProjectorType): string {
  switch (type) {
    case 'source':
      return 'Source';
    case 'scene':
      return 'Scene';
    case 'preview':
      return 'Preview';
    case 'program':
      return 'Program';
    case 'multiview':
      return 'Multiview';
    default:
      return type;
  }
}
