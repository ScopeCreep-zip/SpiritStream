import type { ProfileSettings } from '@spiritstream/types';
import { useThemeStore } from '@/stores/themeStore';
import { useLanguageStore, type Language } from '@/stores/languageStore';
import { useSettingsStore } from '@/stores/settingsStore';

/// Apply UI-only profile settings (theme, language, notifications).
/// Called when settings are updated on the current profile.
export const applyUiSettings = (settings: ProfileSettings): void => {
  if (settings.themeId) {
    useThemeStore.getState().setTheme(settings.themeId);
  }
  if (settings.language) {
    useLanguageStore.getState().setLanguage(settings.language as Language);
  }
  useSettingsStore.getState().setShowNotifications(settings.showNotifications);
};

/// Apply the UI-only consequences of a profile activation (theme, language,
/// notification preference). The backend's `ProfileService::activate()` owns
/// OBS disconnect/reconfigure/auto-connect — the frontend never orchestrates
/// those, it just receives the resulting obs:// events through the
/// `useObsStore` subscription. The frontend just renders.
export const applyProfileSettings = (settings: ProfileSettings): void => {
  applyUiSettings(settings);
};
