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
  | 'audioDevice';

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
 * Union type for all source types
 */
export type Source =
  | RtmpSource
  | MediaFileSource
  | ScreenCaptureSource
  | CameraSource
  | CaptureCardSource
  | AudioDeviceSource;

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
  displayId = ''
): ScreenCaptureSource {
  return {
    type: 'screenCapture',
    id: crypto.randomUUID(),
    name,
    displayId,
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
  }
}
