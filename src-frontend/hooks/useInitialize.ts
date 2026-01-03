import { useEffect, useRef } from 'react';
import { useProfileStore } from '@/stores/profileStore';
import { useStreamStore } from '@/stores/streamStore';
import { api } from '@/lib/tauri';

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

  useEffect(() => {
    if (!initialized.current) {
      initialized.current = true;

      // Load profiles and sync stream state in parallel
      Promise.all([
        loadProfiles(),
        syncWithBackend(),
      ])
        .then(async () => {
          // After profiles are loaded, try to restore last used profile
          try {
            const settings = await api.settings.get();
            if (settings.lastProfile) {
              const profiles = useProfileStore.getState().profiles;
              const exists = profiles.some((p) => p.name === settings.lastProfile);
              if (exists) {
                // Load the last used profile (will trigger password modal if encrypted)
                await loadProfile(settings.lastProfile);
                console.log('[useInitialize] Restored last profile:', settings.lastProfile);
              }
            }
          } catch (error) {
            console.warn('[useInitialize] Failed to restore last profile:', error);
          }
        })
        .catch((error) => {
          console.error('[useInitialize] Initialization failed:', error);
        });
    }
  }, [loadProfiles, loadProfile, syncWithBackend]);
}
