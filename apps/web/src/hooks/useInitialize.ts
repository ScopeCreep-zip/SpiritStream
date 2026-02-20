import { useEffect, useRef } from 'react';
import { useProfileStore } from '@/stores/profileStore';
import { useStreamStore } from '@/stores/streamStore';
import { useThemeStore, initThemeEventListener } from '@/stores/themeStore';
import { useRecordingStore } from '@/stores/recordingStore';
import { useReplayBufferStore } from '@/stores/replayBufferStore';
import { api } from '@/lib/backend';

/**
 * Hook to initialize the application on startup
 * Loads profiles and syncs stream state from the Tauri backend
 * Also auto-restores the last used profile if available
 *
 * Optimized for parallel initialization:
 * - Profile restore is nested inside loadProfiles() to run in parallel with other tasks
 * - Theme setting is nested inside refreshThemes() to avoid sequential awaits
 * - Device discovery is pre-warmed in background to populate cache
 */
export function useInitialize() {
  const initialized = useRef(false);
  const loadProfiles = useProfileStore((state) => state.loadProfiles);
  const loadProfile = useProfileStore((state) => state.loadProfile);
  const syncWithBackend = useStreamStore((state) => state.syncWithBackend);
  const setTheme = useThemeStore((state) => state.setTheme);
  const refreshThemes = useThemeStore((state) => state.refreshThemes);
  const initRecordingPath = useRecordingStore((state) => state.initializeDefaultPath);
  const initReplayBufferPath = useReplayBufferStore((state) => state.initializeDefaultPath);

  useEffect(() => {
    if (!initialized.current) {
      initialized.current = true;

      // Initialize theme event listener for live theme updates
      initThemeEventListener();

      // Single settings fetch shared by profile restore and theme setup
      // Previously these were two separate api.settings.get() calls
      const settingsPromise = api.settings.get().catch(() => null);

      // Run all initialization tasks in parallel for faster startup
      Promise.all([
        // Load profiles, then restore last used profile using shared settings
        loadProfiles().then(async () => {
          try {
            const settings = await settingsPromise;
            if (settings?.lastProfile) {
              const profiles = useProfileStore.getState().profiles;
              const exists = profiles.some((p) => p.name === settings.lastProfile);
              if (exists) {
                await loadProfile(settings.lastProfile);
              }
            }
          } catch {
            // Ignore restore errors - user can manually select profile
          }
        }),
        syncWithBackend(),
        // Refresh themes, then apply stored theme using shared settings
        refreshThemes().then(async () => {
          try {
            const settings = await settingsPromise;
            const storedThemeId = settings?.themeId || useThemeStore.getState().currentThemeId;
            if (storedThemeId) {
              await setTheme(storedThemeId);
              if (settings && !settings.themeId) {
                await api.settings.save({ ...settings, themeId: storedThemeId });
              }
            }
          } catch {
            // Ignore theme errors
          }
        }),
        initRecordingPath(),
        initReplayBufferPath(),
      ]).catch(() => {
        // Initialization errors handled elsewhere
      });
    }
  }, [loadProfiles, loadProfile, syncWithBackend, setTheme, refreshThemes, initRecordingPath, initReplayBufferPath]);
}
