import { useQuery, useMutation, useQueryClient, useIsMutating } from '@tanstack/react-query';
import { useEffect, useCallback } from 'react';
import { api } from '@/lib/backend';
import { useSettingsStore } from '@/stores/settingsStore';
import { useLanguageStore, type Language } from '@/stores/languageStore';
import { useThemeStore } from '@/stores/themeStore';
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
 * Hook to fetch settings data with TanStack Query
 *
 * This provides:
 * - Automatic caching and background refetching
 * - Loading and error states
 * - Multi-client sync support
 */
export function useSettings() {
  const { initFromSettings } = useLanguageStore();

  const query = useQuery({
    queryKey: SETTINGS_QUERY_KEY,
    queryFn: async (): Promise<SettingsData> => {
      // Load settings from backend
      const [backendSettings, profilesPath] = await Promise.all([
        api.settings.get(),
        api.settings.getProfilesPath(),
      ]);

      // Initialize i18n with the saved language
      initFromSettings(backendSettings.language);

      // Sync showNotifications to global store for toast system
      useSettingsStore.getState().setShowNotifications(backendSettings.showNotifications);

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
 * Hook for updating individual settings with optimistic updates
 *
 * This provides:
 * - Instant UI feedback (optimistic update)
 * - Automatic rollback on error
 * - Multi-client sync via invalidation
 */
export function useUpdateSetting() {
  const queryClient = useQueryClient();
  const { setLanguage } = useLanguageStore();

  return useMutation({
    mutationFn: async ({ key, value }: { key: keyof AppSettings; value: unknown }) => {
      const current = queryClient.getQueryData<SettingsData>(SETTINGS_QUERY_KEY);
      if (!current) {
        throw new Error('Settings not loaded');
      }

      const updated: AppSettings = {
        language: current.language,
        startMinimized: current.startMinimized,
        showNotifications: current.showNotifications,
        ffmpegPath: current.ffmpegPath,
        autoDownloadFfmpeg: current.autoDownloadFfmpeg,
        encryptStreamKeys: current.encryptStreamKeys,
        logRetentionDays: current.logRetentionDays,
        themeId: useThemeStore.getState().currentThemeId,
        backendRemoteEnabled: current.backendRemoteEnabled,
        backendUiEnabled: current.backendUiEnabled,
        backendHost: current.backendHost,
        backendPort: current.backendPort,
        backendToken: current.backendToken,
        obsHost: current.obsHost ?? 'localhost',
        obsPort: current.obsPort ?? 4455,
        obsPassword: current.obsPassword ?? '',
        obsUseAuth: current.obsUseAuth ?? false,
        obsDirection: current.obsDirection ?? 'disabled',
        obsAutoConnect: current.obsAutoConnect ?? false,
        lastProfile: current.lastProfile,
        discordWebhookEnabled: current.discordWebhookEnabled ?? false,
        discordWebhookUrl: current.discordWebhookUrl ?? '',
        discordGoLiveMessage: current.discordGoLiveMessage ?? '**Stream is now live!** 🎮\n\nCome join the stream!',
        discordCooldownEnabled: current.discordCooldownEnabled ?? true,
        discordCooldownSeconds: current.discordCooldownSeconds ?? 60,
        discordImagePath: current.discordImagePath ?? '',
        chatTwitchChannel: current.chatTwitchChannel ?? '',
        chatYoutubeChannelId: current.chatYoutubeChannelId ?? '',
        chatYoutubeApiKey: current.chatYoutubeApiKey ?? '',
        chatTwitchSendEnabled: current.chatTwitchSendEnabled ?? false,
        chatYoutubeSendEnabled: current.chatYoutubeSendEnabled ?? false,
        chatSendAllEnabled: current.chatSendAllEnabled ?? true,
        chatCrosspostEnabled: current.chatCrosspostEnabled ?? false,
        // Twitch OAuth account
        twitchOauthAccessToken: current.twitchOauthAccessToken ?? '',
        twitchOauthRefreshToken: current.twitchOauthRefreshToken ?? '',
        twitchOauthExpiresAt: current.twitchOauthExpiresAt ?? 0,
        twitchOauthUserId: current.twitchOauthUserId ?? '',
        twitchOauthUsername: current.twitchOauthUsername ?? '',
        twitchOauthDisplayName: current.twitchOauthDisplayName ?? '',
        // YouTube OAuth account
        youtubeOauthAccessToken: current.youtubeOauthAccessToken ?? '',
        youtubeOauthRefreshToken: current.youtubeOauthRefreshToken ?? '',
        youtubeOauthExpiresAt: current.youtubeOauthExpiresAt ?? 0,
        youtubeOauthChannelId: current.youtubeOauthChannelId ?? '',
        youtubeOauthChannelName: current.youtubeOauthChannelName ?? '',
        // YouTube auth mode
        youtubeUseApiKey: current.youtubeUseApiKey ?? false,
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

      // Handle side effects for specific settings
      if (key === 'language') {
        setLanguage(value as Language);
      }
      if (key === 'showNotifications') {
        useSettingsStore.getState().setShowNotifications(value as boolean);
      }

      return { previousSettings };
    },

    onError: (err, variables, context) => {
      // Rollback to previous value on error
      if (context?.previousSettings) {
        queryClient.setQueryData(SETTINGS_QUERY_KEY, context.previousSettings);

        // Rollback side effects
        if (variables.key === 'language') {
          setLanguage(context.previousSettings.language as Language);
        }
        if (variables.key === 'showNotifications') {
          useSettingsStore.getState().setShowNotifications(
            context.previousSettings.showNotifications
          );
        }
      }
      console.error('Failed to save setting:', err);
    },

    // Note: No onSettled invalidation - optimistic update already has correct data
    // Multi-client sync is handled by useSettingsSync listening for remote events
  });
}

/**
 * Hook for saving multiple settings at once (e.g., after theme change)
 */
export function useSaveSettings() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (updates: Partial<AppSettings>) => {
      const current = queryClient.getQueryData<SettingsData>(SETTINGS_QUERY_KEY);
      if (!current) {
        throw new Error('Settings not loaded');
      }

      const updated: AppSettings = {
        language: current.language,
        startMinimized: current.startMinimized,
        showNotifications: current.showNotifications,
        ffmpegPath: current.ffmpegPath,
        autoDownloadFfmpeg: current.autoDownloadFfmpeg,
        encryptStreamKeys: current.encryptStreamKeys,
        logRetentionDays: current.logRetentionDays,
        themeId: useThemeStore.getState().currentThemeId,
        backendRemoteEnabled: current.backendRemoteEnabled,
        backendUiEnabled: current.backendUiEnabled,
        backendHost: current.backendHost,
        backendPort: current.backendPort,
        backendToken: current.backendToken,
        obsHost: current.obsHost ?? 'localhost',
        obsPort: current.obsPort ?? 4455,
        obsPassword: current.obsPassword ?? '',
        obsUseAuth: current.obsUseAuth ?? false,
        obsDirection: current.obsDirection ?? 'disabled',
        obsAutoConnect: current.obsAutoConnect ?? false,
        lastProfile: current.lastProfile,
        discordWebhookEnabled: current.discordWebhookEnabled ?? false,
        discordWebhookUrl: current.discordWebhookUrl ?? '',
        discordGoLiveMessage: current.discordGoLiveMessage ?? '**Stream is now live!** 🎮\n\nCome join the stream!',
        discordCooldownEnabled: current.discordCooldownEnabled ?? true,
        discordCooldownSeconds: current.discordCooldownSeconds ?? 60,
        discordImagePath: current.discordImagePath ?? '',
        chatTwitchChannel: current.chatTwitchChannel ?? '',
        chatYoutubeChannelId: current.chatYoutubeChannelId ?? '',
        chatYoutubeApiKey: current.chatYoutubeApiKey ?? '',
        chatTwitchSendEnabled: current.chatTwitchSendEnabled ?? false,
        chatYoutubeSendEnabled: current.chatYoutubeSendEnabled ?? false,
        chatSendAllEnabled: current.chatSendAllEnabled ?? true,
        chatCrosspostEnabled: current.chatCrosspostEnabled ?? false,
        // Twitch OAuth account
        twitchOauthAccessToken: current.twitchOauthAccessToken ?? '',
        twitchOauthRefreshToken: current.twitchOauthRefreshToken ?? '',
        twitchOauthExpiresAt: current.twitchOauthExpiresAt ?? 0,
        twitchOauthUserId: current.twitchOauthUserId ?? '',
        twitchOauthUsername: current.twitchOauthUsername ?? '',
        twitchOauthDisplayName: current.twitchOauthDisplayName ?? '',
        // YouTube OAuth account
        youtubeOauthAccessToken: current.youtubeOauthAccessToken ?? '',
        youtubeOauthRefreshToken: current.youtubeOauthRefreshToken ?? '',
        youtubeOauthExpiresAt: current.youtubeOauthExpiresAt ?? 0,
        youtubeOauthChannelId: current.youtubeOauthChannelId ?? '',
        youtubeOauthChannelName: current.youtubeOauthChannelName ?? '',
        // YouTube auth mode
        youtubeUseApiKey: current.youtubeUseApiKey ?? false,
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

    // Note: No onSettled invalidation - optimistic update already has correct data
    // Multi-client sync is handled by useSettingsSync listening for remote events
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
