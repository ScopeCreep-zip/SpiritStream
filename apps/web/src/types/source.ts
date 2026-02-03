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

/**
 * Helper to check if source has video
 */
export function sourceHasVideo(source: Source): boolean {
  switch (source.type) {
    case 'rtmp':
    case 'screenCapture':
    case 'windowCapture':
    case 'gameCapture':
    case 'camera':
    case 'captureCard':
    case 'color':
    case 'text':
    case 'browser':
    case 'nestedScene':
    case 'ndi':
      return true;
    case 'mediaFile':
      return !source.audioOnly;
    case 'mediaPlaylist':
      return true; // Playlists typically contain video
    case 'audioDevice':
      return false;
  }
}

/**
 * Helper to check if source has audio
 * Note: Camera returns false because audio comes from the auto-created linked AudioDeviceSource
 */
export function sourceHasAudio(source: Source): boolean {
  switch (source.type) {
    case 'rtmp':
      return source.captureAudio;
    case 'mediaFile':
      // Media files have audio by default unless explicitly disabled
      return source.captureAudio !== false;
    case 'audioDevice':
      return true;
    case 'captureCard':
      return source.captureAudio;
    case 'ndi':
      return source.captureAudio;
    case 'screenCapture':
      return source.captureAudio;
    // Camera video itself has no audio - audio comes from linked AudioDeviceSource
    case 'camera':
      return false;
    case 'windowCapture':
      return source.captureAudio;
    case 'gameCapture':
      return source.captureAudio;
    case 'mediaPlaylist':
      return source.captureAudio;
    case 'color':
    case 'text':
    case 'browser':
    case 'nestedScene':
      return false;
  }
}

/**
 * Factory functions for creating default sources
 */
export function createDefaultRtmpSource(name = 'RTMP Input'): RtmpSource {
  return {
    type: 'rtmp',
    id: crypto.randomUUID(),
    name,
    bindAddress: '0.0.0.0',
    port: 1935,
    application: 'live',
    captureAudio: true,
  };
}

export function createDefaultMediaFileSource(
  name = 'Media File',
  filePath = ''
): MediaFileSource {
  return {
    type: 'mediaFile',
    id: crypto.randomUUID(),
    name,
    filePath,
    loopPlayback: false,
    audioOnly: false,
    captureAudio: true,
  };
}

export function createDefaultScreenCaptureSource(
  name = 'Screen Capture',
  displayId = '',
  deviceName?: string
): ScreenCaptureSource {
  return {
    type: 'screenCapture',
    id: crypto.randomUUID(),
    name,
    displayId,
    deviceName,
    captureCursor: true,
    captureAudio: false,
    fps: 30,
  };
}

export function createDefaultCameraSource(
  name = 'Camera',
  deviceId = ''
): CameraSource {
  return {
    type: 'camera',
    id: crypto.randomUUID(),
    name,
    deviceId,
    captureAudio: false,
  };
}

export function createDefaultCaptureCardSource(
  name = 'Capture Card',
  deviceId = ''
): CaptureCardSource {
  return {
    type: 'captureCard',
    id: crypto.randomUUID(),
    name,
    deviceId,
    captureAudio: true,
  };
}

export function createDefaultAudioDeviceSource(
  name = 'Audio Input',
  deviceId = ''
): AudioDeviceSource {
  return {
    type: 'audioDevice',
    id: crypto.randomUUID(),
    name,
    deviceId,
  };
}

export function createDefaultColorSource(
  name = 'Color Fill',
  color = '#7C3AED'
): ColorSource {
  return {
    type: 'color',
    id: crypto.randomUUID(),
    name,
    color,
    opacity: 1.0,
  };
}

export function createDefaultTextSource(
  name = 'Text',
  content = ''
): TextSource {
  return {
    type: 'text',
    id: crypto.randomUUID(),
    name,
    content,
    fontFamily: 'Arial',
    fontSize: 48,
    fontWeight: 'normal',
    fontStyle: 'normal',
    textColor: '#FFFFFF',
    backgroundColor: undefined,
    backgroundOpacity: 0.8,
    textAlign: 'center',
    lineHeight: 1.2,
    padding: 16,
    outline: {
      enabled: false,
      color: '#000000',
      width: 2,
    },
  };
}

export function createDefaultBrowserSource(
  name = 'Browser',
  url = ''
): BrowserSource {
  return {
    type: 'browser',
    id: crypto.randomUUID(),
    name,
    url,
    width: 1920,
    height: 1080,
    customCss: undefined,
    refreshInterval: 0,
  };
}

export function createDefaultWindowCaptureSource(
  name = 'Window Capture',
  windowId = '',
  windowTitle = ''
): WindowCaptureSource {
  return {
    type: 'windowCapture',
    id: crypto.randomUUID(),
    name,
    windowId,
    windowTitle,
    captureCursor: true,
    fps: 30,
    captureAudio: false,
  };
}

export function createDefaultMediaPlaylistSource(
  name = 'Media Playlist'
): MediaPlaylistSource {
  return {
    type: 'mediaPlaylist',
    id: crypto.randomUUID(),
    name,
    items: [],
    currentItemIndex: 0,
    autoAdvance: true,
    shuffleMode: 'none',
    fadeBetweenItems: false,
    fadeDurationMs: 500,
    captureAudio: true,
  };
}

export function createDefaultNestedSceneSource(
  name = 'Nested Scene',
  referencedSceneId = ''
): NestedSceneSource {
  return {
    type: 'nestedScene',
    id: crypto.randomUUID(),
    name,
    referencedSceneId,
  };
}

export function createDefaultGameCaptureSource(
  name = 'Game Capture'
): GameCaptureSource {
  return {
    type: 'gameCapture',
    id: crypto.randomUUID(),
    name,
    targetType: 'any',
    captureMode: 'auto',
    captureCursor: false,
    antiCheatHook: false,
    fps: 60,
    captureAudio: false,
  };
}

export function createDefaultNDISource(
  name = 'NDI Source',
  sourceName = ''
): NDISource {
  return {
    type: 'ndi',
    id: crypto.randomUUID(),
    name,
    sourceName,
    lowBandwidth: false,
    receiverName: 'SpiritStream',
    captureAudio: true,
  };
}

/**
 * Get a human-readable label for source type
 */
export function getSourceTypeLabel(type: SourceType): string {
  switch (type) {
    case 'rtmp':
      return 'RTMP Input';
    case 'mediaFile':
      return 'Media File';
    case 'screenCapture':
      return 'Screen Capture';
    case 'windowCapture':
      return 'Window Capture';
    case 'gameCapture':
      return 'Game Capture';
    case 'camera':
      return 'Camera';
    case 'captureCard':
      return 'Capture Card';
    case 'audioDevice':
      return 'Audio Device';
    case 'color':
      return 'Color';
    case 'text':
      return 'Text';
    case 'browser':
      return 'Browser';
    case 'mediaPlaylist':
      return 'Media Playlist';
    case 'nestedScene':
      return 'Nested Scene';
    case 'ndi':
      return 'NDI Source';
  }
}

/**
 * Get icon name for source type (for Lucide icons)
 */
export function getSourceTypeIcon(type: SourceType): string {
  switch (type) {
    case 'rtmp':
      return 'Radio';
    case 'mediaFile':
      return 'Film';
    case 'screenCapture':
      return 'Monitor';
    case 'windowCapture':
      return 'AppWindow';
    case 'gameCapture':
      return 'Gamepad2';
    case 'camera':
      return 'Camera';
    case 'captureCard':
      return 'Usb';
    case 'audioDevice':
      return 'Mic';
    case 'color':
      return 'Palette';
    case 'text':
      return 'Type';
    case 'browser':
      return 'Globe';
    case 'mediaPlaylist':
      return 'ListVideo';
    case 'nestedScene':
      return 'Layers';
    case 'ndi':
      return 'Network';
  }
}

/**
 * Check if source renders via pure CSS (no WebRTC needed)
 */
export function isClientSideSource(source: Source): boolean {
  // Game capture and NDI require backend rendering, not client-side
  return source.type === 'color' || source.type === 'text' || source.type === 'browser' || source.type === 'nestedScene';
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

/**
 * Factory functions for audio filters
 */
export function createCompressorFilter(): CompressorFilter {
  return {
    id: crypto.randomUUID(),
    type: 'compressor',
    enabled: true,
    order: 0,
    threshold: -20,
    ratio: 4,
    attack: 5,
    release: 50,
    outputGain: 0,
  };
}

export function createNoiseGateFilter(): NoiseGateFilter {
  return {
    id: crypto.randomUUID(),
    type: 'noiseGate',
    enabled: true,
    order: 0,
    threshold: -40,
    attack: 5,
    hold: 100,
    release: 100,
  };
}

export function createNoiseSuppressionFilter(): NoiseSuppressionFilter {
  return {
    id: crypto.randomUUID(),
    type: 'noiseSuppression',
    enabled: true,
    order: 0,
    level: 50,
  };
}

export function createGainFilter(): GainFilter {
  return {
    id: crypto.randomUUID(),
    type: 'gain',
    enabled: true,
    order: 0,
    gain: 0,
  };
}

export function createExpanderFilter(): ExpanderFilter {
  return {
    id: crypto.randomUUID(),
    type: 'expander',
    enabled: true,
    order: 0,
    threshold: -40,
    ratio: 2,
    attack: 5,
    release: 100,
  };
}

/**
 * Get human-readable label for audio filter type
 */
export function getAudioFilterLabel(type: AudioFilterType): string {
  switch (type) {
    case 'compressor':
      return 'Compressor';
    case 'noiseGate':
      return 'Noise Gate';
    case 'noiseSuppression':
      return 'Noise Suppression';
    case 'gain':
      return 'Gain';
    case 'expander':
      return 'Expander';
  }
}

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

/**
 * Factory functions for video filters
 */
export function createChromaKeyFilter(): ChromaKeyFilter {
  return {
    id: crypto.randomUUID(),
    type: 'chromaKey',
    enabled: true,
    order: 0,
    keyColor: '#00FF00', // Green screen default
    similarity: 400,
    smoothness: 80,
    keySpill: 100,
  };
}

export function createColorKeyFilter(): ColorKeyFilter {
  return {
    id: crypto.randomUUID(),
    type: 'colorKey',
    enabled: true,
    order: 0,
    keyColor: '#00FF00',
    similarity: 400,
    smoothness: 80,
  };
}

export function createColorCorrectionFilter(): ColorCorrectionFilter {
  return {
    id: crypto.randomUUID(),
    type: 'colorCorrection',
    enabled: true,
    order: 0,
    brightness: 0,
    contrast: 0,
    saturation: 1,
    gamma: 1,
    hue: 0,
  };
}

export function createLUTFilter(): LUTFilter {
  return {
    id: crypto.randomUUID(),
    type: 'lut',
    enabled: true,
    order: 0,
    lutFile: '',
    intensity: 1,
  };
}

export function createBlurFilter(): BlurFilter {
  return {
    id: crypto.randomUUID(),
    type: 'blur',
    enabled: true,
    order: 0,
    blurType: 'gaussian',
    size: 10,
  };
}

export function createSharpenFilter(): SharpenFilter {
  return {
    id: crypto.randomUUID(),
    type: 'sharpen',
    enabled: true,
    order: 0,
    amount: 1,
  };
}

export function createScrollFilter(): ScrollFilter {
  return {
    id: crypto.randomUUID(),
    type: 'scroll',
    enabled: true,
    order: 0,
    horizontalSpeed: 0,
    verticalSpeed: 50,
    loop: true,
  };
}

export function createMaskFilter(): MaskFilter {
  return {
    id: crypto.randomUUID(),
    type: 'mask',
    enabled: true,
    order: 0,
    maskImage: '',
    maskType: 'alpha',
    invert: false,
  };
}

export function createTransform3DFilter(): Transform3DFilter {
  return {
    id: crypto.randomUUID(),
    type: 'transform3d',
    enabled: true,
    order: 0,
    rotationX: 0,
    rotationY: 0,
    rotationZ: 0,
    perspective: 1000,
    positionX: 0,
    positionY: 0,
    positionZ: 0,
  };
}

/**
 * Get human-readable label for video filter type
 */
export function getVideoFilterLabel(type: VideoFilterType): string {
  switch (type) {
    case 'chromaKey':
      return 'Chroma Key';
    case 'colorKey':
      return 'Color Key';
    case 'colorCorrection':
      return 'Color Correction';
    case 'lut':
      return 'LUT';
    case 'blur':
      return 'Blur';
    case 'sharpen':
      return 'Sharpen';
    case 'scroll':
      return 'Scroll';
    case 'mask':
      return 'Image Mask';
    case 'transform3d':
      return '3D Transform';
  }
}

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
