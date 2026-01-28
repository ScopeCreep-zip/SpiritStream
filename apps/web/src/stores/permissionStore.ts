// Permission Store
// Manages cross-platform permission state for camera, microphone, and screen recording

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { api } from '../lib/backend';
import { isTauri } from '../lib/backend/env';

/**
 * Helper to invoke commands via Tauri IPC when in desktop mode.
 *
 * Permission operations MUST use Tauri IPC directly when running in the
 * Tauri webview. The HTTP server layer doesn't trigger system dialogs -
 * only the desktop layer (apps/desktop/src-tauri/src/permissions.rs) has
 * the proper crabcamera/scap calls that show macOS permission prompts.
 */
async function permissionInvoke<T>(
  command: string,
  args?: Record<string, unknown>
): Promise<T> {
  if (isTauri()) {
    // Use Tauri IPC directly - this reaches the desktop layer
    return invoke<T>(command, args);
  }
  // Fall back to HTTP API for web/docker modes
  return api.invoke<T>(command, args);
}

// Permission states match the Rust PermissionState enum
export type PermissionStateValue =
  | 'granted'
  | 'denied'
  | 'not_determined'
  | 'restricted'
  | 'unknown';

export interface PermissionStatus {
  camera: PermissionStateValue;
  microphone: PermissionStateValue;
  screenRecording: PermissionStateValue;
}

export type PermissionType = 'camera' | 'microphone' | 'screen_recording';
export type SourcePermissionType = 'camera' | 'microphone' | 'screenRecording';

// Platform type from backend
export type Platform = 'macos' | 'windows' | 'linux' | 'unknown';

interface PermissionState {
  // Permission status
  status: PermissionStatus;
  // Current platform
  platform: Platform;
  // Loading state
  isChecking: boolean;
  isRequesting: boolean;
  // Last check timestamp
  lastChecked: number | null;
  // Error state
  error: string | null;

  // Actions
  initialize: () => Promise<void>;
  checkPermissions: () => Promise<PermissionStatus>;
  requestPermission: (permission: PermissionType) => Promise<boolean>;
  getPermissionGuidance: (permission: PermissionType) => Promise<string>;
  hasPermission: (permission: SourcePermissionType) => boolean;
  canPromptPermission: (permission: SourcePermissionType) => boolean;
  needsPermission: (sourceType: string) => SourcePermissionType | null;
}

// Default status - assume unknown until we check
const defaultStatus: PermissionStatus = {
  camera: 'unknown',
  microphone: 'unknown',
  screenRecording: 'unknown',
};

export const usePermissionStore = create<PermissionState>((set, get) => ({
  status: defaultStatus,
  platform: 'unknown',
  isChecking: false,
  isRequesting: false,
  lastChecked: null,
  error: null,

  initialize: async () => {
    try {
      // Get platform first - use Tauri IPC when available
      const platform = await permissionInvoke<string>('get_platform');
      set({ platform: platform as Platform });

      // Then check permissions
      await get().checkPermissions();
    } catch (error) {
      console.warn('Failed to initialize permissions:', error);
      // On error, assume permissions are granted to not block functionality
      set({
        status: {
          camera: 'granted',
          microphone: 'granted',
          screenRecording: 'granted',
        },
        error: String(error),
      });
    }
  },

  checkPermissions: async () => {
    set({ isChecking: true, error: null });

    try {
      // Call Tauri command to check permissions - uses IPC when in Tauri
      const response = await permissionInvoke<PermissionStatus>('check_permissions');

      if (response) {
        set({
          status: response,
          lastChecked: Date.now(),
          isChecking: false,
        });
        return response;
      }
    } catch (error) {
      console.warn('Failed to check permissions:', error);
      set({ error: String(error) });
      // On error, assume permissions are granted to not block functionality
    }

    set({ isChecking: false });
    return get().status;
  },

  requestPermission: async (permission: PermissionType) => {
    const { status } = get();

    // Map permission type to status key
    const statusKey: SourcePermissionType =
      permission === 'screen_recording' ? 'screenRecording' : permission;

    // Already granted
    if (status[statusKey] === 'granted') {
      return true;
    }

    // If denied or restricted, we can't re-request programmatically
    if (status[statusKey] === 'denied' || status[statusKey] === 'restricted') {
      console.warn(`Permission ${permission} is ${status[statusKey]}, cannot re-request`);
      return false;
    }

    set({ isRequesting: true, error: null });

    try {
      // Call Tauri command to request permission - uses IPC when in Tauri
      // permType uses snake_case to match Rust enum
      // IMPORTANT: Must use Tauri IPC directly to trigger system dialogs on macOS
      const granted = await permissionInvoke<boolean>('request_permission', {
        permType: permission,
      });

      // Re-check permissions after request to get updated status
      await get().checkPermissions();

      set({ isRequesting: false });
      return granted;
    } catch (error) {
      console.warn(`Failed to request ${permission} permission:`, error);
      set({ isRequesting: false, error: String(error) });
    }

    return false;
  },

  getPermissionGuidance: async (permission: PermissionType) => {
    try {
      // Use Tauri IPC when available for consistent permission handling
      const guidance = await permissionInvoke<string>('get_permission_guidance', {
        permType: permission,
      });
      return guidance;
    } catch (error) {
      console.warn('Failed to get permission guidance:', error);
      return 'Please check your system settings to enable this permission.';
    }
  },

  hasPermission: (permission: SourcePermissionType) => {
    const { status, platform } = get();

    // If permission is granted, return true
    if (status[permission] === 'granted') {
      return true;
    }

    // On Windows, screen recording doesn't need permission
    if (platform === 'windows' && permission === 'screenRecording') {
      return true;
    }

    // If unknown, we'll try and let the OS handle it
    if (status[permission] === 'unknown') {
      return true;
    }

    return false;
  },

  canPromptPermission: (permission: SourcePermissionType) => {
    const { status } = get();
    // Can only prompt if permission is not_determined or unknown
    return status[permission] === 'not_determined' || status[permission] === 'unknown';
  },

  needsPermission: (sourceType: string) => {
    const { status, platform } = get();

    // Windows screen recording doesn't need permission
    if (platform === 'windows' && sourceType === 'screenCapture') {
      return null;
    }

    switch (sourceType) {
      case 'camera':
        return status.camera === 'granted' || status.camera === 'unknown' ? null : 'camera';
      case 'screenCapture':
        return status.screenRecording === 'granted' || status.screenRecording === 'unknown'
          ? null
          : 'screenRecording';
      case 'audioDevice':
        return status.microphone === 'granted' || status.microphone === 'unknown'
          ? null
          : 'microphone';
      default:
        return null;
    }
  },
}));

// Helper hook for permission-aware actions
export function usePermissionCheck() {
  const {
    checkPermissions,
    requestPermission,
    getPermissionGuidance,
    hasPermission,
    canPromptPermission,
    needsPermission,
    status,
    platform,
    isChecking,
    isRequesting,
  } = usePermissionStore();

  const ensurePermission = async (
    sourceType: string
  ): Promise<{
    granted: boolean;
    permission?: SourcePermissionType;
    guidance?: string;
  }> => {
    // First check current status
    await checkPermissions();

    const needed = needsPermission(sourceType);
    if (!needed) {
      return { granted: true };
    }

    // Map sourceType permission name to command permission type
    const permType: PermissionType =
      needed === 'screenRecording' ? 'screen_recording' : needed;

    // Check if we can prompt
    if (!canPromptPermission(needed)) {
      // Permission is denied or restricted - get guidance
      const guidance = await getPermissionGuidance(permType);
      return { granted: false, permission: needed, guidance };
    }

    // Request the permission
    const granted = await requestPermission(permType);

    if (!granted) {
      const guidance = await getPermissionGuidance(permType);
      return { granted: false, permission: needed, guidance };
    }

    return { granted: true, permission: needed };
  };

  return {
    checkPermissions,
    requestPermission,
    getPermissionGuidance,
    hasPermission,
    canPromptPermission,
    needsPermission,
    ensurePermission,
    status,
    platform,
    isChecking,
    isRequesting,
  };
}
