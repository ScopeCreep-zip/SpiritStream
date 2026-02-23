/**
 * Audio utilities module
 *
 * Exports pure JavaScript audio level store, rendering utilities,
 * and the meter coordinator for high-performance audio metering
 * that bypasses React's render cycle.
 */

export {
  updateLevels,
  updateLevelsBinary,
  getTrackLevel,
  getMasterLevel,
  getPeakHold,
  getMasterPeakHold,
  getVersion,
  resetLevels,
  type StereoLevel,
  type AudioLevelsData,
} from './audioLevelStore';

export {
  drawMeter,
  LABEL_WIDTH,
  BAR_WIDTH,
  ARROW_WIDTH,
  ARROW_HEIGHT,
  PADDING_Y,
  METER_HEIGHT,
  TOTAL_HEIGHT,
  TOTAL_WIDTH,
  type MeterRenderOptions,
} from './meterRenderer';

export {
  registerMeter,
  unregisterMeter,
  pauseCoordinator,
  resumeCoordinator,
  type MeterConfig,
  type MeterRegistration,
} from './meterCoordinator';

export {
  decodeAudioFrame,
  AUDIO_FRAME_MAGIC,
  AUDIO_FRAME_VERSION,
  type DecodedAudioFrame,
  type DecodedTrackLevel,
} from './binaryCodec';
