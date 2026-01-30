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
  | 'camera'
  | 'captureCard'
  | 'audioDevice'
  | 'color'
  | 'text'
  | 'browser';

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
}

/**
 * Media file source - local video/audio file
 */
export interface MediaFileSource extends BaseSource {
  type: 'mediaFile';
  filePath: string;
  loopPlayback: boolean;
  audioOnly?: boolean;
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
 * Camera source - webcam or video capture device
 */
export interface CameraSource extends BaseSource {
  type: 'camera';
  deviceId: string;
  width?: number;
  height?: number;
  fps?: number;
}

/**
 * Capture card source - HDMI/SDI capture devices
 */
export interface CaptureCardSource extends BaseSource {
  type: 'captureCard';
  deviceId: string;
  inputFormat?: string;
}

/**
 * Audio device source - microphone, line-in, etc.
 */
export interface AudioDeviceSource extends BaseSource {
  type: 'audioDevice';
  deviceId: string;
  channels?: number;
  sampleRate?: number;
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
 * Union type for all source types
 */
export type Source =
  | RtmpSource
  | MediaFileSource
  | ScreenCaptureSource
  | CameraSource
  | CaptureCardSource
  | AudioDeviceSource
  | ColorSource
  | TextSource
  | BrowserSource;

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
 * Helper to check if source has video
 */
export function sourceHasVideo(source: Source): boolean {
  switch (source.type) {
    case 'rtmp':
    case 'screenCapture':
    case 'camera':
    case 'captureCard':
    case 'color':
    case 'text':
    case 'browser':
      return true;
    case 'mediaFile':
      return !source.audioOnly;
    case 'audioDevice':
      return false;
  }
}

/**
 * Helper to check if source has audio
 */
export function sourceHasAudio(source: Source): boolean {
  switch (source.type) {
    case 'rtmp':
    case 'mediaFile':
    case 'captureCard':
    case 'audioDevice':
      return true;
    case 'screenCapture':
      return source.captureAudio;
    case 'camera':
    case 'color':
    case 'text':
    case 'browser':
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
  }
}

/**
 * Check if source renders via pure CSS (no WebRTC needed)
 */
export function isClientSideSource(source: Source): boolean {
  return source.type === 'color' || source.type === 'text' || source.type === 'browser';
}
