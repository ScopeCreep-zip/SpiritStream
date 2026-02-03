/**
 * Audio utilities module
 *
 * Exports pure JavaScript audio level store and rendering utilities
 * for high-performance audio metering that bypasses React's render cycle.
 */

export {
  updateLevels,
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
