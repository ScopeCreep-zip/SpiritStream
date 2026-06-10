import { describe, it, expect, beforeEach, vi } from 'vitest';
import type { ProfileSettings } from '@spiritstream/types';

// N5: applySettings slice. This is the UI-only fan-out a profile
// activation triggers — theme, language, notification preference. The
// load-bearing contract is that it ONLY touches the three UI stores and
// never orchestrates backend side-effects (OBS connect/reconfigure is
// owned by the Rust core; the frontend just renders the resulting
// events). Each setter is guarded: themeId/language only apply when
// present, showNotifications always applies.

const { setTheme, setLanguage, setShowNotifications } = vi.hoisted(() => ({
  setTheme: vi.fn(),
  setLanguage: vi.fn(),
  setShowNotifications: vi.fn(),
}));

vi.mock('@/stores/themeStore', () => ({
  useThemeStore: { getState: () => ({ setTheme }) },
}));
vi.mock('@/stores/languageStore', () => ({
  useLanguageStore: { getState: () => ({ setLanguage }) },
}));
vi.mock('@/stores/settingsStore', () => ({
  useSettingsStore: { getState: () => ({ setShowNotifications }) },
}));

import { applyUiSettings, applyProfileSettings } from './applySettings';

function makeSettings(overrides: Partial<ProfileSettings> = {}): ProfileSettings {
  return {
    themeId: 'spirit-dark',
    language: 'en',
    showNotifications: true,
    ...overrides,
  } as unknown as ProfileSettings;
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe('applyUiSettings', () => {
  it('applies theme, language, and notification preference', () => {
    applyUiSettings(makeSettings({ themeId: 'spirit-light', language: 'de', showNotifications: false }));
    expect(setTheme).toHaveBeenCalledWith('spirit-light');
    expect(setLanguage).toHaveBeenCalledWith('de');
    expect(setShowNotifications).toHaveBeenCalledWith(false);
  });

  it('skips theme and language when absent but always sets notifications', () => {
    applyUiSettings(makeSettings({ themeId: undefined, language: undefined, showNotifications: true }));
    expect(setTheme).not.toHaveBeenCalled();
    expect(setLanguage).not.toHaveBeenCalled();
    expect(setShowNotifications).toHaveBeenCalledWith(true);
  });
});

describe('applyProfileSettings', () => {
  it('delegates to applyUiSettings (no backend orchestration)', () => {
    applyProfileSettings(makeSettings({ themeId: 'spirit-dark', language: 'fr' }));
    expect(setTheme).toHaveBeenCalledWith('spirit-dark');
    expect(setLanguage).toHaveBeenCalledWith('fr');
    expect(setShowNotifications).toHaveBeenCalledTimes(1);
  });
});
