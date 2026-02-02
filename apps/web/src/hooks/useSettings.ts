import { useQuery, useMutation, useQueryClient, useIsMutating } from '@tanstack/react-query';
import { useEffect, useCallback } from 'react';
import { api } from '@/lib/backend';
import type { AppSettings } from '@/types/api';

/**
 * Extended settings state including derived data
 */
export interface SettingsData extends AppSettings {
  profileStoragePath: string;
  ffmpegVersion: string;
}

/**
 * Query key for settings
 */
export const SETTINGS_QUERY_KEY = ['settings'] as const;

/**
 * Query key for FFmpeg version (expensive, separate query)
 */
export const FFMPEG_VERSION_QUERY_KEY = ['ffmpeg-version'] as const;

/**
 * Hook to fetch global settings data with TanStack Query
 *
 * NOTE: Profile-specific settings (theme, language, integrations) have been
 * moved to ProfileSettings and are now managed through the profile store.
 * This hook only handles global, app-wide settings.
 */
export function useSettings() {
  const query = useQuery({
    queryKey: SETTINGS_QUERY_KEY,
    queryFn: async (): Promise<SettingsData> => {
      // Load settings from backend
      const [backendSettings, profilesPath] = await Promise.all([
        api.settings.get(),
        api.settings.getProfilesPath(),
      ]);

      return {
        ...backendSettings,
        profileStoragePath: profilesPath,
        ffmpegVersion: '', // Loaded separately to avoid blocking
      };
    },
    staleTime: 60_000, // Consider fresh for 1 minute
  });

  return query;
}

/**
 * Separate query for FFmpeg version (expensive operation)
 * This prevents the expensive FFmpeg test from blocking settings load
 */
export function useFfmpegVersion() {
  return useQuery({
    queryKey: FFMPEG_VERSION_QUERY_KEY,
    queryFn: async () => {
      try {
        const version = await api.system.testFfmpeg();
        const path = await api.system.getFfmpegPath();
        return { version, path: path || '' };
      } catch {
        return { version: '', path: '' };
      }
    },
    staleTime: 5 * 60_000, // Consider fresh for 5 minutes
    retry: false, // Don't retry FFmpeg detection
  });
}

/**
 * Hook for updating individual global settings with optimistic updates
 *
 * NOTE: Profile-specific settings (theme, language, OBS, Discord) should be
 * updated through the profile store, not through this hook.
 */
export function useUpdateSetting() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({ key, value }: { key: keyof AppSettings; value: unknown }) => {
      const current = queryClient.getQueryData<SettingsData>(SETTINGS_QUERY_KEY);
      if (!current) {
        throw new Error('Settings not loaded');
      }

      // Build global settings object (only non-legacy fields)
      const updated: AppSettings = {
        startMinimized: current.startMinimized,
        ffmpegPath: current.ffmpegPath,
        autoDownloadFfmpeg: current.autoDownloadFfmpeg,
        logRetentionDays: current.logRetentionDays,
        lastProfile: current.lastProfile,
        twitchOauthAccessToken: current.twitchOauthAccessToken ?? '',
        twitchOauthRefreshToken: current.twitchOauthRefreshToken ?? '',
        twitchOauthExpiresAt: current.twitchOauthExpiresAt ?? 0,
        twitchOauthUserId: current.twitchOauthUserId ?? '',
        twitchOauthUsername: current.twitchOauthUsername ?? '',
        twitchOauthDisplayName: current.twitchOauthDisplayName ?? '',
        youtubeOauthAccessToken: current.youtubeOauthAccessToken ?? '',
        youtubeOauthRefreshToken: current.youtubeOauthRefreshToken ?? '',
        youtubeOauthExpiresAt: current.youtubeOauthExpiresAt ?? 0,
        youtubeOauthChannelId: current.youtubeOauthChannelId ?? '',
        youtubeOauthChannelName: current.youtubeOauthChannelName ?? '',
        [key]: value,
      };

      await api.settings.save(updated);
      return { key, value };
    },

    onMutate: async ({ key, value }) => {
      // Cancel any outgoing refetches to avoid overwriting optimistic update
      await queryClient.cancelQueries({ queryKey: SETTINGS_QUERY_KEY });

      // Snapshot previous value for rollback
      const previousSettings = queryClient.getQueryData<SettingsData>(SETTINGS_QUERY_KEY);

      // Optimistically update cache immediately
      queryClient.setQueryData<SettingsData>(SETTINGS_QUERY_KEY, (old) => {
        if (!old) return old;
        return { ...old, [key]: value };
      });

      return { previousSettings };
    },

    onError: (err, _variables, context) => {
      // Rollback to previous value on error
      if (context?.previousSettings) {
        queryClient.setQueryData(SETTINGS_QUERY_KEY, context.previousSettings);
      }
      console.error('Failed to save setting:', err);
    },
  });
}

/**
 * Hook for saving multiple global settings at once
 *
 * NOTE: Profile-specific settings should be updated through the profile store.
 */
export function useSaveSettings() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (updates: Partial<AppSettings>) => {
      const current = queryClient.getQueryData<SettingsData>(SETTINGS_QUERY_KEY);
      if (!current) {
        throw new Error('Settings not loaded');
      }

      // Build global settings object (only non-legacy fields)
      const updated: AppSettings = {
        startMinimized: current.startMinimized,
        ffmpegPath: current.ffmpegPath,
        autoDownloadFfmpeg: current.autoDownloadFfmpeg,
        logRetentionDays: current.logRetentionDays,
        lastProfile: current.lastProfile,
        twitchOauthAccessToken: current.twitchOauthAccessToken ?? '',
        twitchOauthRefreshToken: current.twitchOauthRefreshToken ?? '',
        twitchOauthExpiresAt: current.twitchOauthExpiresAt ?? 0,
        twitchOauthUserId: current.twitchOauthUserId ?? '',
        twitchOauthUsername: current.twitchOauthUsername ?? '',
        twitchOauthDisplayName: current.twitchOauthDisplayName ?? '',
        youtubeOauthAccessToken: current.youtubeOauthAccessToken ?? '',
        youtubeOauthRefreshToken: current.youtubeOauthRefreshToken ?? '',
        youtubeOauthExpiresAt: current.youtubeOauthExpiresAt ?? 0,
        youtubeOauthChannelId: current.youtubeOauthChannelId ?? '',
        youtubeOauthChannelName: current.youtubeOauthChannelName ?? '',
        ...updates,
      };

      await api.settings.save(updated);
      return updates;
    },

    onMutate: async (updates) => {
      await queryClient.cancelQueries({ queryKey: SETTINGS_QUERY_KEY });
      const previousSettings = queryClient.getQueryData<SettingsData>(SETTINGS_QUERY_KEY);

      queryClient.setQueryData<SettingsData>(SETTINGS_QUERY_KEY, (old) => {
        if (!old) return old;
        return { ...old, ...updates };
      });

      return { previousSettings };
    },

    onError: (err, _variables, context) => {
      if (context?.previousSettings) {
        queryClient.setQueryData(SETTINGS_QUERY_KEY, context.previousSettings);
      }
      console.error('Failed to save settings:', err);
    },
  });
}

/**
 * Hook to listen for remote settings changes and invalidate cache
 * Only invalidates if no local mutation is in progress
 */
export function useSettingsSync() {
  const queryClient = useQueryClient();
  const isMutating = useIsMutating({ mutationKey: ['settings'] });

  useEffect(() => {
    const handleRemoteChange = () => {
      // Only invalidate if we're not currently mutating
      // This prevents the "reload" effect when we save our own changes
      if (isMutating === 0) {
        queryClient.invalidateQueries({ queryKey: SETTINGS_QUERY_KEY });
      }
    };

    window.addEventListener('backend:settings_changed', handleRemoteChange);
    return () => {
      window.removeEventListener('backend:settings_changed', handleRemoteChange);
    };
  }, [queryClient, isMutating]);
}

/**
 * Hook to manually refresh FFmpeg version (after download completes)
 */
export function useRefreshFfmpegVersion() {
  const queryClient = useQueryClient();

  return useCallback(() => {
    queryClient.invalidateQueries({ queryKey: FFMPEG_VERSION_QUERY_KEY });
  }, [queryClient]);
}
