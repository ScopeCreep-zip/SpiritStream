/**
 * Binary Audio Level Protocol
 *
 * Compact binary format for real-time audio levels over WebSocket.
 * Replaces JSON serialization (~3KB) with binary (~300 bytes) for 6 tracks.
 *
 * Wire format (little-endian):
 *   [1 byte magic 0xAF] [1 byte version] [2 bytes track count]
 *   [MASTER_BLOCK: 9 × f32 = 36 bytes]
 *   [TRACK_BLOCK × N: 1 byte id_len, id_len bytes UTF-8 id, 9 × f32 + 1 byte flags = 37 bytes]
 *
 * f32 fields (per block): rmsL, rmsR, peakL, peakR, peakDb, inputPeakL, inputPeakR, rms, peak
 * flags byte: bit 0 = clipping, bit 1 = has inputPeakL/R
 */

export const AUDIO_FRAME_MAGIC = 0xAF;
export const AUDIO_FRAME_VERSION = 1;
const HEADER_SIZE = 4; // magic + version + trackCount(u16)
const BLOCK_FLOATS = 9; // rmsL, rmsR, peakL, peakR, peakDb, inputPeakL, inputPeakR, rms, peak
const BLOCK_FLOAT_BYTES = BLOCK_FLOATS * 4; // 36 bytes
const MASTER_BLOCK_SIZE = BLOCK_FLOAT_BYTES + 1; // 36 + 1 flags byte = 37 bytes

export interface DecodedTrackLevel {
  rmsL: number;
  rmsR: number;
  peakL: number;
  peakR: number;
  peakDb: number;
  inputPeakL: number;
  inputPeakR: number;
  rms: number;
  peak: number;
  clipping: boolean;
  hasInputPeak: boolean;
}

export interface DecodedAudioFrame {
  master: DecodedTrackLevel;
  tracks: Map<string, DecodedTrackLevel>;
}

// Reusable TextDecoder for track ID strings
const textDecoder = new TextDecoder();

function readBlock(view: DataView, offset: number): { level: DecodedTrackLevel; bytesRead: number } {
  const rmsL = view.getFloat32(offset, true);
  const rmsR = view.getFloat32(offset + 4, true);
  const peakL = view.getFloat32(offset + 8, true);
  const peakR = view.getFloat32(offset + 12, true);
  const peakDb = view.getFloat32(offset + 16, true);
  const inputPeakL = view.getFloat32(offset + 20, true);
  const inputPeakR = view.getFloat32(offset + 24, true);
  const rms = view.getFloat32(offset + 28, true);
  const peak = view.getFloat32(offset + 32, true);
  const flags = view.getUint8(offset + 36);

  return {
    level: {
      rmsL,
      rmsR,
      peakL,
      peakR,
      peakDb,
      inputPeakL,
      inputPeakR,
      rms,
      peak,
      clipping: (flags & 0x01) !== 0,
      hasInputPeak: (flags & 0x02) !== 0,
    },
    bytesRead: MASTER_BLOCK_SIZE,
  };
}

export function decodeAudioFrame(buffer: ArrayBuffer): DecodedAudioFrame | null {
  if (buffer.byteLength < HEADER_SIZE + MASTER_BLOCK_SIZE) return null;

  const view = new DataView(buffer);

  // Validate header
  if (view.getUint8(0) !== AUDIO_FRAME_MAGIC) return null;
  if (view.getUint8(1) !== AUDIO_FRAME_VERSION) return null;

  const trackCount = view.getUint16(2, true);
  let offset = HEADER_SIZE;

  // Read master block
  const masterResult = readBlock(view, offset);
  offset += masterResult.bytesRead;

  // Read track blocks
  const tracks = new Map<string, DecodedTrackLevel>();
  const uint8 = new Uint8Array(buffer);

  for (let i = 0; i < trackCount; i++) {
    if (offset >= buffer.byteLength) break;

    // Read track ID: 1 byte length + UTF-8 bytes
    const idLen = view.getUint8(offset);
    offset += 1;

    if (offset + idLen + MASTER_BLOCK_SIZE > buffer.byteLength) break;

    const trackId = textDecoder.decode(uint8.subarray(offset, offset + idLen));
    offset += idLen;

    const trackResult = readBlock(view, offset);
    offset += trackResult.bytesRead;

    tracks.set(trackId, trackResult.level);
  }

  return {
    master: masterResult.level,
    tracks,
  };
}
