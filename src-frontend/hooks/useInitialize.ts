import { useEffect, useRef } from 'react';
import { useProfileStore } from '@/stores/profileStore';

/**
 * Hook to initialize the application on startup
 * Loads profiles from the Tauri backend
 */
export function useInitialize() {
  const initialized = useRef(false);
  const loadProfiles = useProfileStore((state) => state.loadProfiles);

  useEffect(() => {
    if (!initialized.current) {
      initialized.current = true;
      loadProfiles();
    }
  }, [loadProfiles]);
}
