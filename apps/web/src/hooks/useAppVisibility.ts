import { useEffect } from 'react';
import { api } from '@/lib/backend';
import { pauseAudioMeterWorker, resumeAudioMeterWorker } from '@/lib/audio/audioMeterWorkerBridge';
import { useWebRTCConnectionStore } from '@/stores/webrtcConnectionStore';

/**
 * Notifies the backend when the browser tab becomes hidden or visible.
 * This allows the server to throttle preview encoding and audio level
 * updates while the UI is not visible, saving CPU/GPU/battery.
 *
 * Also pauses/resumes:
 * - Audio meter render loop in the Web Worker
 * - WebRTC video track decoding (connections stay alive, video decoding stops)
 */
export function useAppVisibility(): void {
  useEffect(() => {
    const handler = (): void => {
      const idle = document.hidden;

      // Notify backend to throttle preview encoding + audio levels
      api.invoke('set_idle_mode', { idle }).catch(() => {
        // Silently ignore if backend doesn't support this yet
      });

      if (idle) {
        // Pause audio meter worker render loop
        pauseAudioMeterWorker();
        // Pause WebRTC video tracks — stops video decoding without closing connections.
        // go2rtc stops sending when RTCP feedback ceases, saving CPU on both ends.
        useWebRTCConnectionStore.getState().pauseAllVideo();
      } else {
        // Resume audio meter worker render loop
        resumeAudioMeterWorker();
        // Resume WebRTC video tracks
        useWebRTCConnectionStore.getState().resumeAllVideo();
      }
    };
    document.addEventListener('visibilitychange', handler);
    return () => document.removeEventListener('visibilitychange', handler);
  }, []);
}
