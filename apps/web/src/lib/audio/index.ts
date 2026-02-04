/**
 * Audio utilities module
 *
 * Exports pure JavaScript audio level store and rendering utilities
 * for high-performance audio metering that bypasses React's render cycle.
 *
 * Also exports Web Worker-based rendering for complete main thread offloading.
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

export {
  registerMeterRenderer,
  shouldUpdatePeakDb,
  shouldCheckClipping,
  getActiveRendererCount,
} from './audioAnimationManager';

// Web Worker-based rendering (OffscreenCanvas)
export {
  initAudioMeterWorker,
  terminateAudioMeterWorker,
  registerMeterCanvas,
  unregisterMeterCanvas,
  updateMeterConfig,
  forwardAudioData,
  isWorkerReady,
  hasSharedMemory,
  getMasterLevelFromShared,
  getSharedVersion,
} from './audioMeterWorkerBridge';
