import { useEffect, useRef } from 'react';
import { events, backendMode } from '@/lib/backend';
import { useProfileStore } from '@/stores/profileStore';

interface ProfileChangedPayload {
  action: 'saved' | 'deleted';
  name: string;
}

/**
 * Hook that listens for backend data change events and refreshes stores.
 * This enables real-time synchronization between multiple clients
 * (e.g., Tauri app and web browser) connected to the same backend.
 */
export function useDataSync() {
  const loadProfiles = useProfileStore((state) => state.loadProfiles);
  const currentProfileName = useProfileStore((state) => state.current?.name);
  const loadProfile = useProfileStore((state) => state.loadProfile);

  // Use refs to avoid stale closures in event handlers
  const currentProfileNameRef = useRef(currentProfileName);
  currentProfileNameRef.current = currentProfileName;

  useEffect(() => {
    // Only needed in HTTP mode - Tauri mode already has direct state updates
    if (backendMode !== 'http') return;

    const unsubscribers: Array<() => void> = [];

    // Listen for profile changes from other clients
    events
      .on<ProfileChangedPayload>('profile_changed', (payload) => {
        // Reload the profile list
        loadProfiles();

        // If the current profile was updated by another client, reload it
        if (payload.action === 'saved' && payload.name === currentProfileNameRef.current) {
          loadProfile(payload.name);
        }

        // If the current profile was deleted, it will be handled by loadProfiles
        // which will show the profile is no longer available
      })
      .then((unsub) => unsubscribers.push(unsub))
      .catch(() => {});

    // Listen for settings changes from other clients
    // Settings are typically loaded on-demand in the Settings view,
    // so we emit a custom event that the Settings view can listen to
    events
      .on('settings_changed', () => {
        window.dispatchEvent(new CustomEvent('backend:settings_changed'));
      })
      .then((unsub) => unsubscribers.push(unsub))
      .catch(() => {});

    return () => {
      unsubscribers.forEach((unsub) => unsub());
    };
  }, [loadProfiles, loadProfile]);
}
