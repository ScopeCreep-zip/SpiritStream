import { useEffect, useRef } from 'react';
import { useProfileStore } from '@/stores/profileStore';
import { useStreamStore } from '@/stores/streamStore';
import { useThemeStore, initThemeEventListener } from '@/stores/themeStore';
import { api } from '@/lib/backend';

/**
 * Hook to initialize the application on startup
 * Loads profiles and syncs stream state from the Tauri backend
 * Also auto-restores the last used profile if available
 */
export function useInitialize() {
  const initialized = useRef(false);
  const loadProfiles = useProfileStore((state) => state.loadProfiles);
  const loadProfile = useProfileStore((state) => state.loadProfile);
  const syncWithBackend = useStreamStore((state) => state.syncWithBackend);
  const setTheme = useThemeStore((state) => state.setTheme);
  const refreshThemes = useThemeStore((state) => state.refreshThemes);

  useEffect(() => {
    if (!initialized.current) {
      initialized.current = true;

      // Load profiles, themes, and sync stream state in parallel
      // NOTE: refreshThemes is called here (not in themeStore hydration) to ensure
      // the server is ready - App component's health check runs before this hook
      // Also initialize theme event listener for live theme updates
      initThemeEventListener();
      Promise.all([loadProfiles(), syncWithBackend(), refreshThemes()])
        .then(async () => {
          // After profiles are loaded, try to restore last used profile
          try {
            const settings = await api.settings.get();
            const storedThemeId = settings.themeId || useThemeStore.getState().currentThemeId;
            if (storedThemeId) {
              await setTheme(storedThemeId);
              if (!settings.themeId) {
                await api.settings.save({ ...settings, themeId: storedThemeId });
              }
            }
            if (settings.lastProfile) {
              const profiles = useProfileStore.getState().profiles;
              const exists = profiles.some((p) => p.name === settings.lastProfile);
              if (exists) {
                // Load the last used profile (will trigger password modal if encrypted)
                await loadProfile(settings.lastProfile);
              }
            }
          } catch {
            // Ignore restore errors - user can manually select profile
          }
        })
        .catch(() => {
          // Initialization errors handled elsewhere
        });
    }
  }, [loadProfiles, loadProfile, syncWithBackend]);
}
