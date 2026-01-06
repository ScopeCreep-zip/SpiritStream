import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { listen } from '@tauri-apps/api/event';
import { api } from '@/lib/tauri';
import type { ThemeSummary, ThemeMode } from '@/types/theme';

interface ThemeState {
  currentThemeId: string;
  themes: ThemeSummary[];
  currentTokens?: Record<string, string>;

  // Computed property
  currentMode: ThemeMode;

  // Actions
  setTheme: (themeId: string) => Promise<void>;
  refreshThemes: () => Promise<void>;
}

const DEFAULT_THEME_LIGHT = 'spirit-light';
const DEFAULT_THEME_DARK = 'spirit-dark';
const THEME_STYLE_ID = 'spiritstream-theme-overrides';

function getSystemTheme(): ThemeMode {
  if (typeof window === 'undefined') return 'light';
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

function applyTheme(themeId: string, mode: ThemeMode, tokens?: Record<string, string>) {
  if (typeof document === 'undefined') return;

  document.documentElement.setAttribute('data-theme', mode);
  document.documentElement.setAttribute('data-theme-id', themeId);

  // Only apply overrides when tokens are provided
  if (tokens) {
    setThemeOverrides(themeId, mode, tokens);
  } else {
    clearThemeOverrides();
  }
}

function setThemeOverrides(themeId: string, mode: ThemeMode, tokens: Record<string, string>) {
  if (typeof document === 'undefined') return;
  const styleId = THEME_STYLE_ID;
  let style = document.getElementById(styleId) as HTMLStyleElement | null;
  if (!style) {
    style = document.createElement('style');
    style.id = styleId;
    document.head.appendChild(style);
  }

  const entries = Object.entries(tokens)
    .map(([key, value]) => `  ${key}: ${value};`)
    .join('\n');

  // Single selector for this specific theme + mode
  style.textContent = `:root[data-theme-id="${themeId}"][data-theme="${mode}"] {\n${entries}\n}`;
}

function clearThemeOverrides() {
  if (typeof document === 'undefined') return;
  const style = document.getElementById(THEME_STYLE_ID);
  if (style && style.parentNode) {
    style.parentNode.removeChild(style);
  }
}

function migrateOldThemeFormat(): { themeId: string } | null {
  try {
    const oldData = localStorage.getItem('spiritstream-theme');
    if (!oldData) return null;

    const old = JSON.parse(oldData);
    // Old format: { theme: 'light'|'dark'|'system', themeId: 'spirit' }
    // New format: { currentThemeId: 'spirit-light' }

    if (old.currentThemeId) return null; // Already new format

    const mode: ThemeMode =
      old.theme === 'system' ? getSystemTheme() : old.theme === 'dark' ? 'dark' : 'light';
    const oldThemeId = old.themeId || 'spirit';

    // Construct new theme ID if needed
    const newThemeId =
      oldThemeId.endsWith('-light') || oldThemeId.endsWith('-dark')
        ? oldThemeId
        : `${oldThemeId}-${mode}`;

    console.log(`Migrating theme from old format: ${old.themeId} (${old.theme}) -> ${newThemeId}`);

    return { themeId: newThemeId };
  } catch (error) {
    console.error('Failed to migrate old theme format:', error);
    return null;
  }
}

export const useThemeStore = create<ThemeState>()(
  persist(
    (set, get) => ({
      currentThemeId: DEFAULT_THEME_LIGHT,
      themes: [],
      currentTokens: undefined,
      currentMode: 'light',

      setTheme: async (themeId) => {
        try {
          const themes = get().themes;
          const theme = themes.find((t) => t.id === themeId);
          if (!theme) {
            console.error(`Theme ${themeId} not found`);
            return;
          }

          let tokens: Record<string, string> | undefined;
          if (!(theme.source === 'builtin' && theme.id.startsWith('spirit-'))) {
            tokens = await api.theme.getTokens(themeId);
          }

          applyTheme(themeId, theme.mode, tokens);
          set({ currentThemeId: themeId, currentMode: theme.mode, currentTokens: tokens });
        } catch (error) {
          console.error('Failed to load theme:', error);
          // Fall back to default
          const fallback = themeId.endsWith('-dark') ? DEFAULT_THEME_DARK : DEFAULT_THEME_LIGHT;
          const fallbackMode: ThemeMode = fallback.endsWith('-dark') ? 'dark' : 'light';
          applyTheme(fallback, fallbackMode, undefined);
          set({ currentThemeId: fallback, currentMode: fallbackMode, currentTokens: undefined });
        }
      },

      refreshThemes: async () => {
        try {
          const themes = await api.theme.list();
          set({ themes });

          // Check if current theme still exists
          const { currentThemeId } = get();
          if (!themes.find((theme) => theme.id === currentThemeId)) {
            // Fall back to default
            const fallback = currentThemeId.endsWith('-dark') ? DEFAULT_THEME_DARK : DEFAULT_THEME_LIGHT;
            await get().setTheme(fallback);
          }
        } catch (error) {
          console.error('Failed to refresh themes:', error);
          set({
            themes: [
              { id: DEFAULT_THEME_LIGHT, name: 'Spirit Light', mode: 'light', source: 'builtin' },
              { id: DEFAULT_THEME_DARK, name: 'Spirit Dark', mode: 'dark', source: 'builtin' },
            ],
          });
        }
      },
    }),
    {
      name: 'spiritstream-theme',
      partialize: (state) => ({
        currentThemeId: state.currentThemeId,
      }),
      onRehydrateStorage: () => (state) => {
        if (state) {
          // Try to migrate old format
          const migrated = migrateOldThemeFormat();
          if (migrated) {
            state.currentThemeId = migrated.themeId;
            // Clear old format from localStorage
            const oldData = localStorage.getItem('spiritstream-theme');
            if (oldData) {
              try {
                const parsed = JSON.parse(oldData);
                delete parsed.theme;
                delete parsed.themeId;
                localStorage.setItem('spiritstream-theme', JSON.stringify(parsed));
              } catch (e) {
                console.error('Failed to clean up old theme format:', e);
              }
            }
          }

          // Load themes and apply current theme
          state.refreshThemes().then(() => {
            const { currentThemeId, themes } = state;
            const theme = themes.find((t) => t.id === currentThemeId);
            if (theme) {
              state.setTheme(currentThemeId);
            }
          });
        }
      },
    }
  )
);

if (typeof window !== 'undefined') {
  // Listen for theme file changes from backend
  listen<ThemeSummary[]>('themes_updated', (event) => {
    const themes = event.payload;
    useThemeStore.setState({ themes });
    const state = useThemeStore.getState();
    if (!themes.find((theme) => theme.id === state.currentThemeId)) {
      const fallback = state.currentThemeId.endsWith('-dark')
        ? DEFAULT_THEME_DARK
        : DEFAULT_THEME_LIGHT;
      state.setTheme(fallback);
    }
  }).catch((error) => {
    console.error('Failed to listen for theme updates:', error);
  });
}
