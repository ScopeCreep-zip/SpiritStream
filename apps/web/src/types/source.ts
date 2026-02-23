/**
 * Source types for multi-input streaming
 * Mirrors server/src/models/source.rs
 */

/**
 * Source type discriminator
 */
export type SourceType =
  | 'rtmp'
  | 'mediaFile'
  | 'screenCapture'
  | 'windowCapture'
  | 'gameCapture'
  | 'camera'
  | 'captureCard'
  | 'audioDevice'
  | 'color'
  | 'text'
  | 'browser'
  | 'mediaPlaylist'
  | 'nestedScene'
  | 'ndi';

/**
 * Base source interface
 */
interface BaseSource {
  id: string;
  name: string;
}

/**
 * RTMP input source - incoming RTMP stream
 */
export interface RtmpSource extends BaseSource {
  type: 'rtmp';
  bindAddress: string;
  port: number;
  application: string;
  /** Whether to capture audio from this source */
  captureAudio: boolean;
}

/**
 * Media file source - local video/audio file
 */
export interface MediaFileSource extends BaseSource {
  type: 'mediaFile';
  filePath: string;
  loopPlayback: boolean;
  audioOnly?: boolean;
  /** Whether to capture audio from this media file (default: true) */
  captureAudio?: boolean;
}

/**
 * Screen capture source - captures a display
 */
export interface ScreenCaptureSource extends BaseSource {
  type: 'screenCapture';
  displayId: string;
  /** The actual device name as reported by the OS (e.g., "Capture screen 0" on macOS) */
  deviceName?: string;
  captureCursor: boolean;
  captureAudio: boolean;
  fps: number;
  /** Target capture resolution. Default '1080p'. Use 'captured' for native display resolution. */
  captureResolution?: '480p' | '720p' | '1080p' | '1440p' | '2160p' | 'captured';
}

/**
 * Window capture source - captures a specific application window
 */
export interface WindowCaptureSource extends BaseSource {
  type: 'windowCapture';
  windowId: string;
  windowTitle: string;
  /** Process name or app name */
  processName?: string;
  captureCursor: boolean;
  fps: number;
  /** Whether to capture window audio (macOS/Windows) */
  captureAudio: boolean;
  /** Target capture resolution. Default '1080p'. Use 'captured' for native display resolution. */
  captureResolution?: '480p' | '720p' | '1080p' | '1440p' | '2160p' | 'captured';
}

/**
 * Game capture source - captures games with hardware acceleration
 * Platform-specific: DXGI (Windows), ScreenCaptureKit (macOS), PipeWire (Linux)
 */
export interface GameCaptureSource extends BaseSource {
  type: 'gameCapture';
  /** 'any' captures any fullscreen game, 'specific' targets a window/process */
  targetType: 'any' | 'specific';
  /** Window title to capture (when targetType is 'specific') */
  windowTitle?: string;
  /** Process name to capture (when targetType is 'specific') */
  processName?: string;
  /** Capture method - 'auto' selects best for platform */
  captureMode: 'auto' | 'bitblt' | 'dxgi' | 'opengl';
  /** Whether to include cursor in capture */
  captureCursor: boolean;
  /** Enable anti-cheat compatible hooking (may reduce performance) */
  antiCheatHook: boolean;
  /** Capture framerate */
  fps: number;
  /** Whether to capture game audio */
  captureAudio: boolean;
}

/**
 * Camera source - webcam or video capture device
 */
export interface CameraSource extends BaseSource {
  type: 'camera';
  deviceId: string;
  width?: number;
  height?: number;
  fps?: number;
  /** Whether to capture audio from built-in microphone */
  captureAudio: boolean;
  /** Auto-discovered linked audio device ID (from CameraDevice)
   * When captureAudio is true, an AudioDeviceSource will be auto-created for this device */
  linkedAudioDeviceId?: string;
}

/**
 * Capture card source - HDMI/SDI capture devices
 */
export interface CaptureCardSource extends BaseSource {
  type: 'captureCard';
  deviceId: string;
  inputFormat?: string;
  /** Whether to capture audio from this source */
  captureAudio: boolean;
}

/**
 * Audio device source - microphone, line-in, etc.
 */
export interface AudioDeviceSource extends BaseSource {
  type: 'audioDevice';
  deviceId: string;
  channels?: number;
  sampleRate?: number;
  /** If this was auto-created as linked audio for another source (e.g., camera)
   * When the parent source is deleted, this source should also be deleted */
  linkedToSourceId?: string;
}

/**
 * Color source - solid color fill
 */
export interface ColorSource extends BaseSource {
  type: 'color';
  color: string; // Hex color: '#FF5733'
  opacity: number; // 0.0 - 1.0
}

/**
 * Text source - text overlay with styling
 */
export interface TextSource extends BaseSource {
  type: 'text';
  content: string;
  fontFamily: string;
  fontSize: number;
  fontWeight: 'normal' | 'bold';
  fontStyle: 'normal' | 'italic';
  textColor: string;
  backgroundColor?: string;
  backgroundOpacity: number;
  textAlign: 'left' | 'center' | 'right';
  lineHeight: number;
  padding: number;
  outline?: {
    enabled: boolean;
    color: string;
    width: number;
  };
}

/**
 * Browser source - web page iframe
 */
export interface BrowserSource extends BaseSource {
  type: 'browser';
  url: string;
  width: number; // Viewport width (default: 1920)
  height: number; // Viewport height (default: 1080)
  customCss?: string; // Optional CSS injection
  refreshInterval?: number; // Seconds, 0 = manual only
  refreshToken?: string; // Changed to trigger manual refresh
}

/**
 * Media playlist source - plays multiple media files in sequence
 */
export interface MediaPlaylistSource extends BaseSource {
  type: 'mediaPlaylist';
  items: PlaylistItem[];
  currentItemIndex: number;
  autoAdvance: boolean;
  shuffleMode: 'none' | 'all' | 'repeat-one';
  fadeBetweenItems: boolean;
  fadeDurationMs?: number;
  /** Whether to capture audio from playlist items */
  captureAudio: boolean;
}

/**
 * Playlist item for media playlist source
 */
export interface PlaylistItem {
  id: string;
  filePath: string;
  duration?: number; // Duration in seconds (auto-detected)
  name?: string; // Display name (defaults to filename)
}

/**
 * Nested scene source - embeds another scene
 */
export interface NestedSceneSource extends BaseSource {
  type: 'nestedScene';
  referencedSceneId: string;
}

/**
 * NDI source - receives video over network via NDI protocol
 * Requires NDI SDK/runtime to be installed
 */
export interface NDISource extends BaseSource {
  type: 'ndi';
  /** Name of the NDI source to receive */
  sourceName: string;
  /** Optional specific IP address (auto-discovers if not set) */
  ipAddress?: string;
  /** Use low bandwidth mode (lower quality, less network usage) */
  lowBandwidth: boolean;
  /** Name to identify this receiver on the network */
  receiverName: string;
  /** Whether to capture audio from this source */
  captureAudio: boolean;
}

/**
 * Union type for all source types
 */
export type Source =
  | RtmpSource
  | MediaFileSource
  | ScreenCaptureSource
  | WindowCaptureSource
  | GameCaptureSource
  | CameraSource
  | CaptureCardSource
  | AudioDeviceSource
  | ColorSource
  | TextSource
  | BrowserSource
  | MediaPlaylistSource
  | NestedSceneSource
  | NDISource;

// Device discovery result types

/**
 * Available resolution for a device
 */
export interface Resolution {
  width: number;
  height: number;
  fps: number[];
}

/**
 * Discovered camera device
 */
export interface CameraDevice {
  deviceId: string;
  name: string;
  resolutions: Resolution[];
  /** Auto-discovered linked audio device ID (e.g., camera's built-in microphone) */
  linkedAudioDeviceId?: string;
  /** Name of the linked audio device */
  linkedAudioDeviceName?: string;
}

/**
 * Discovered display for screen capture
 */
export interface DisplayInfo {
  displayId: string;
  name: string;
  /** The actual device name as reported by the OS (e.g., "Capture screen 0" on macOS) */
  deviceName?: string;
  width: number;
  height: number;
  isPrimary: boolean;
}

/**
 * Discovered audio input device
 */
export interface AudioInputDevice {
  deviceId: string;
  name: string;
  channels: number;
  sampleRate: number;
  isDefault: boolean;
}

/**
 * Discovered capture card device
 */
export interface CaptureCardDevice {
  deviceId: string;
  name: string;
  inputs: string[];
}

/**
 * Discovered window for window capture
 */
export interface WindowInfo {
  windowId: string;
  title: string;
  processName?: string;
  appName?: string;
  width?: number;
  height?: number;
}

// ---------------------------------------------------------------------------
// Re-exports from sourceRegistry (single source of truth)
//
// These were previously 14 factory functions + 6 switch statements inline here.
// Now delegated to @/lib/sourceRegistry for maintainability.
// Existing imports from '@/types/source' continue to work unchanged.
// ---------------------------------------------------------------------------
export {
  sourceHasVideo,
  sourceHasAudio,
  getSourceTypeLabel,
  getSourceTypeIcon,
  isClientSideSource,
  createDefaultSource,
  SOURCE_REGISTRY,
  type SourceCategory,
  type SourceTypeEntry,
} from '@/lib/sourceRegistry';

// Legacy factory functions — thin wrappers around createDefaultSource for
// backward compatibility. New code should use createDefaultSource() directly.
import { createDefaultSource } from '@/lib/sourceRegistry';

export function createDefaultRtmpSource(name = 'RTMP Input'): RtmpSource {
  return createDefaultSource('rtmp', { name });
}

export function createDefaultMediaFileSource(name = 'Media File', filePath = ''): MediaFileSource {
  return createDefaultSource('mediaFile', { name, filePath });
}

export function createDefaultScreenCaptureSource(name = 'Screen Capture', displayId = '', deviceName?: string): ScreenCaptureSource {
  return createDefaultSource('screenCapture', { name, displayId, deviceName });
}

export function createDefaultCameraSource(name = 'Camera', deviceId = ''): CameraSource {
  return createDefaultSource('camera', { name, deviceId });
}

export function createDefaultCaptureCardSource(name = 'Capture Card', deviceId = ''): CaptureCardSource {
  return createDefaultSource('captureCard', { name, deviceId });
}

export function createDefaultAudioDeviceSource(name = 'Audio Input', deviceId = ''): AudioDeviceSource {
  return createDefaultSource('audioDevice', { name, deviceId });
}

export function createDefaultColorSource(name = 'Color Fill', color = '#7C3AED'): ColorSource {
  return createDefaultSource('color', { name, color });
}

export function createDefaultTextSource(name = 'Text', content = ''): TextSource {
  return createDefaultSource('text', { name, content });
}

export function createDefaultBrowserSource(name = 'Browser', url = ''): BrowserSource {
  return createDefaultSource('browser', { name, url });
}

export function createDefaultWindowCaptureSource(name = 'Window Capture', windowId = '', windowTitle = ''): WindowCaptureSource {
  return createDefaultSource('windowCapture', { name, windowId, windowTitle });
}

export function createDefaultMediaPlaylistSource(name = 'Media Playlist'): MediaPlaylistSource {
  return createDefaultSource('mediaPlaylist', { name });
}

export function createDefaultNestedSceneSource(name = 'Nested Scene', referencedSceneId = ''): NestedSceneSource {
  return createDefaultSource('nestedScene', { name, referencedSceneId });
}

export function createDefaultGameCaptureSource(name = 'Game Capture'): GameCaptureSource {
  return createDefaultSource('gameCapture', { name });
}

export function createDefaultNDISource(name = 'NDI Source', sourceName = ''): NDISource {
  return createDefaultSource('ndi', { name, sourceName });
}

// ============================================================================
// AUDIO FILTERS
// ============================================================================

/**
 * Audio filter type discriminator
 */
export type AudioFilterType =
  | 'compressor'
  | 'noiseGate'
  | 'noiseSuppression'
  | 'gain'
  | 'expander';

/**
 * Base audio filter interface
 */
interface BaseAudioFilter {
  id: string;
  type: AudioFilterType;
  enabled: boolean;
  order: number; // Position in filter chain
}

/**
 * Compressor filter - reduces dynamic range
 */
export interface CompressorFilter extends BaseAudioFilter {
  type: 'compressor';
  threshold: number; // dB (-60 to 0)
  ratio: number; // 1:1 to 32:1
  attack: number; // ms (0-500)
  release: number; // ms (0-1000)
  outputGain: number; // dB (-30 to +30)
  /** Optional sidechain source for audio ducking */
  sidechainSourceId?: string;
}

/**
 * Noise Gate filter - cuts audio below threshold
 */
export interface NoiseGateFilter extends BaseAudioFilter {
  type: 'noiseGate';
  threshold: number; // dB (-60 to 0)
  attack: number; // ms (0-100)
  hold: number; // ms (0-500)
  release: number; // ms (0-1000)
}

/**
 * Noise Suppression filter - removes background noise
 */
export interface NoiseSuppressionFilter extends BaseAudioFilter {
  type: 'noiseSuppression';
  level: number; // Suppression strength (0-100)
}

/**
 * Gain filter - adjusts volume level
 */
export interface GainFilter extends BaseAudioFilter {
  type: 'gain';
  gain: number; // dB (-30 to +30)
}

/**
 * Expander filter - increases dynamic range below threshold
 */
export interface ExpanderFilter extends BaseAudioFilter {
  type: 'expander';
  threshold: number; // dB (-60 to 0)
  ratio: number; // 1:1 to 10:1
  attack: number; // ms (0-100)
  release: number; // ms (0-500)
}

/**
 * Union type for all audio filters
 */
export type AudioFilter =
  | CompressorFilter
  | NoiseGateFilter
  | NoiseSuppressionFilter
  | GainFilter
  | ExpanderFilter;

// ---------------------------------------------------------------------------
// Re-exports from filterRegistry (single source of truth)
// ---------------------------------------------------------------------------
export {
  createAudioFilter,
  createVideoFilter,
  getAudioFilterLabel,
  getVideoFilterLabel,
  AUDIO_FILTER_REGISTRY,
  VIDEO_FILTER_REGISTRY,
} from '@/lib/filterRegistry';

import { createAudioFilter } from '@/lib/filterRegistry';

// Legacy named factory functions — wrappers for backward compat
export function createCompressorFilter(): CompressorFilter { return createAudioFilter('compressor'); }
export function createNoiseGateFilter(): NoiseGateFilter { return createAudioFilter('noiseGate'); }
export function createNoiseSuppressionFilter(): NoiseSuppressionFilter { return createAudioFilter('noiseSuppression'); }
export function createGainFilter(): GainFilter { return createAudioFilter('gain'); }
export function createExpanderFilter(): ExpanderFilter { return createAudioFilter('expander'); }

/**
 * All available audio filter types
 */
export const AUDIO_FILTER_TYPES: AudioFilterType[] = [
  'gain',
  'compressor',
  'noiseGate',
  'noiseSuppression',
  'expander',
];

// ============================================================================
// SOURCE-LEVEL AUDIO CONFIG (OBS pattern: audio config lives on source)
// ============================================================================

/**
 * Audio monitoring mode — how audio is routed for preview listening
 */
export type MonitoringType = 'none' | 'monitorOnly' | 'monitorAndOutput';

/**
 * Fader curve type — how the volume fader maps to gain
 * Mirrors server/src/models/source.rs FaderCurve
 */
export type FaderCurve = 'cubic' | 'linear' | 'sine';

/**
 * Per-source audio configuration (OBS parity: source-level, not scene-level).
 * Mirrors server/src/models/source.rs SourceAudioConfig
 */
export interface SourceAudioConfig {
  /** Volume multiplier (0.0-20.0, 1.0 = unity gain) */
  volume: number;
  muted: boolean;
  solo: boolean;
  /** Sync offset in milliseconds (positive = delay audio) */
  syncOffsetMs: number;
  monitoringType: MonitoringType;
  /** Bitmask of which output tracks receive this source's audio (bits 0-5 = tracks 1-6) */
  trackBitmask: number;
  /** Stereo balance (-1.0 = full left, 0.0 = center, 1.0 = full right) */
  balance: number;
  /** Fader curve type (how volume fader maps to gain). Default: 'cubic' */
  faderCurve?: FaderCurve;
  audioFilters: AudioFilter[];
  /** Config version — incremented on any audio config change for mixer hot-reload detection */
  configVersion?: number;
}

/**
 * Create a default source audio config
 */
export function createDefaultSourceAudioConfig(): SourceAudioConfig {
  return {
    volume: 1.0,
    muted: false,
    solo: false,
    syncOffsetMs: 0,
    monitoringType: 'none',
    trackBitmask: 0b000001, // Track 1 only
    balance: 0.0,
    audioFilters: [],
    configVersion: 0,
  };
}

// ============================================================================
// VIDEO FILTERS
// ============================================================================

/**
 * Video filter type discriminator
 */
export type VideoFilterType =
  | 'chromaKey'
  | 'colorKey'
  | 'colorCorrection'
  | 'lut'
  | 'blur'
  | 'sharpen'
  | 'scroll'
  | 'mask'
  | 'transform3d';

/**
 * Base video filter interface
 */
interface BaseVideoFilter {
  id: string;
  type: VideoFilterType;
  enabled: boolean;
  order: number; // Position in filter chain
}

/**
 * Chroma Key filter - green screen removal
 */
export interface ChromaKeyFilter extends BaseVideoFilter {
  type: 'chromaKey';
  keyColor: string; // Hex color to remove
  similarity: number; // 0-1000 (how close to key color)
  smoothness: number; // 0-1000 (edge smoothing)
  keySpill: number; // 0-1000 (color spill reduction)
}

/**
 * Color Key filter - remove specific color
 */
export interface ColorKeyFilter extends BaseVideoFilter {
  type: 'colorKey';
  keyColor: string; // Hex color to remove
  similarity: number; // 0-1000
  smoothness: number; // 0-1000
}

/**
 * Color Correction filter - adjust colors
 */
export interface ColorCorrectionFilter extends BaseVideoFilter {
  type: 'colorCorrection';
  brightness: number; // -1 to 1
  contrast: number; // -1 to 1
  saturation: number; // 0 to 3
  gamma: number; // 0.1 to 4
  hue: number; // -180 to 180
}

/**
 * LUT filter - color grading via lookup table
 */
export interface LUTFilter extends BaseVideoFilter {
  type: 'lut';
  lutFile: string; // Path to .cube or .3dl file
  intensity: number; // 0-1 blend with original
}

/**
 * Blur filter - gaussian or box blur
 */
export interface BlurFilter extends BaseVideoFilter {
  type: 'blur';
  blurType: 'box' | 'gaussian';
  size: number; // Blur radius (1-100)
}

/**
 * Sharpen filter - increase edge contrast
 */
export interface SharpenFilter extends BaseVideoFilter {
  type: 'sharpen';
  amount: number; // 0-10
}

/**
 * Scroll filter - scrolling content
 */
export interface ScrollFilter extends BaseVideoFilter {
  type: 'scroll';
  horizontalSpeed: number; // Pixels per second (-1000 to 1000)
  verticalSpeed: number; // Pixels per second (-1000 to 1000)
  loop: boolean;
}

/**
 * Mask filter - apply image mask
 */
export interface MaskFilter extends BaseVideoFilter {
  type: 'mask';
  maskImage: string; // Path to mask image
  maskType: 'alpha' | 'luminance';
  invert: boolean;
}

/**
 * 3D Transform filter - perspective transform
 */
export interface Transform3DFilter extends BaseVideoFilter {
  type: 'transform3d';
  rotationX: number; // Degrees (-180 to 180)
  rotationY: number; // Degrees (-180 to 180)
  rotationZ: number; // Degrees (-180 to 180)
  perspective: number; // Distance (100-5000)
  positionX: number; // Offset
  positionY: number; // Offset
  positionZ: number; // Offset (depth)
}

/**
 * Union type for all video filters
 */
export type VideoFilter =
  | ChromaKeyFilter
  | ColorKeyFilter
  | ColorCorrectionFilter
  | LUTFilter
  | BlurFilter
  | SharpenFilter
  | ScrollFilter
  | MaskFilter
  | Transform3DFilter;

// Legacy named video filter factory functions — wrappers for backward compat
import { createVideoFilter } from '@/lib/filterRegistry';

export function createChromaKeyFilter(): ChromaKeyFilter { return createVideoFilter('chromaKey'); }
export function createColorKeyFilter(): ColorKeyFilter { return createVideoFilter('colorKey'); }
export function createColorCorrectionFilter(): ColorCorrectionFilter { return createVideoFilter('colorCorrection'); }
export function createLUTFilter(): LUTFilter { return createVideoFilter('lut'); }
export function createBlurFilter(): BlurFilter { return createVideoFilter('blur'); }
export function createSharpenFilter(): SharpenFilter { return createVideoFilter('sharpen'); }
export function createScrollFilter(): ScrollFilter { return createVideoFilter('scroll'); }
export function createMaskFilter(): MaskFilter { return createVideoFilter('mask'); }
export function createTransform3DFilter(): Transform3DFilter { return createVideoFilter('transform3d'); }

/**
 * All available video filter types
 */
export const VIDEO_FILTER_TYPES: VideoFilterType[] = [
  'chromaKey',
  'colorKey',
  'colorCorrection',
  'lut',
  'blur',
  'sharpen',
  'scroll',
  'mask',
  'transform3d',
];
