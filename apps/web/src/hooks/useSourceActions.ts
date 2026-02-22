/**
 * useSourceActions
 *
 * Consolidates the "add source with audio track" workflow that was
 * previously duplicated in AddSourceModal.tsx.
 *
 * Handles:
 * 1. Adding source to profile (backend + local state)
 * 2. Camera linked audio device creation
 * 3. Audio track creation for sources that produce audio
 * 4. Auto-save after audio track changes
 */
import { useCallback } from 'react';
import { useProfileStore } from '@/stores/profileStore';
import { useDeviceStore } from '@/stores/deviceStore';
import type { Source, AudioDeviceSource, CameraSource } from '@/types/source';
import { sourceHasAudio } from '@/lib/sourceRegistry';
import { createDefaultAudioTrack } from '@/types/scene';
import { useShallow } from 'zustand/shallow';

interface AddSourceOptions {
  /** Profile name to add the source to */
  profileName: string;
  /** The source to add */
  source: Source;
  /** Scene ID for audio track creation (uses activeSceneId if omitted) */
  sceneId?: string;
  /** Encryption password if profile is encrypted */
  password?: string;
}

interface SourceActions {
  /** Add a source to the profile with proper audio track setup */
  addSourceToScene: (options: AddSourceOptions) => Promise<void>;
}

export function useSourceActions(): SourceActions {
  const { addSource, addCurrentAudioTrack, saveProfile, current } = useProfileStore(
    useShallow((s) => ({
      addSource: s.addSource,
      addCurrentAudioTrack: s.addCurrentAudioTrack,
      saveProfile: s.saveProfile,
      current: s.current,
    }))
  );
  const cameras = useDeviceStore((s) => s.devices.cameras);

  const addSourceToScene = useCallback(
    async ({ profileName, source, sceneId, password }: AddSourceOptions) => {
      const targetSceneId = sceneId ?? current?.activeSceneId;

      // 1. Add source to profile (backend + local state)
      await addSource(profileName, source, password);

      // 2. Camera linked audio: create separate AudioDeviceSource for the camera's mic
      if (
        source.type === 'camera' &&
        (source as CameraSource).captureAudio &&
        (source as CameraSource).linkedAudioDeviceId
      ) {
        const cam = source as CameraSource;
        const cameraDevice = cameras.find((c) => c.deviceId === cam.deviceId);
        const linkedAudioSource: AudioDeviceSource = {
          type: 'audioDevice',
          id: crypto.randomUUID(),
          name: `${source.name} Audio`,
          deviceId: cam.linkedAudioDeviceId!,
          linkedToSourceId: source.id,
          channels: cameraDevice?.linkedAudioDeviceName?.toLowerCase().includes('stereo') ? 2 : 2,
          sampleRate: 48000,
        };

        await addSource(profileName, linkedAudioSource, password);

        // Add audio track for linked audio source
        if (targetSceneId) {
          addCurrentAudioTrack(targetSceneId, createDefaultAudioTrack(linkedAudioSource.id));
        }
      }

      // 3. Add audio track for the source itself (non-camera audio sources)
      if (sourceHasAudio(source) && targetSceneId) {
        addCurrentAudioTrack(targetSceneId, createDefaultAudioTrack(source.id));
      }

      // 4. Save profile if audio tracks were modified
      const hasAudioChanges =
        sourceHasAudio(source) ||
        (source.type === 'camera' &&
          (source as CameraSource).captureAudio &&
          (source as CameraSource).linkedAudioDeviceId);
      if (hasAudioChanges && targetSceneId) {
        saveProfile();
      }
    },
    [addSource, addCurrentAudioTrack, saveProfile, current?.activeSceneId, cameras]
  );

  return { addSourceToScene };
}
