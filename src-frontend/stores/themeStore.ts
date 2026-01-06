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

function inferMode(themeId: string, fallback: ThemeMode): ThemeMode {
  if (themeId.endsWith('-dark')) return 'dark';
  if (themeId.endsWith('-light')) return 'light';
  return fallback;
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

    const parsed = JSON.parse(oldData);
    if (parsed && typeof parsed === 'object' && 'state' in parsed) {
      const state = (parsed as { state?: { currentThemeId?: string } }).state;
      if (state?.currentThemeId) {
        return null; // Already persisted in the current format
      }
    }

    const legacy = (parsed && typeof parsed === 'object' && 'state' in parsed
      ? (parsed as { state?: Record<string, unknown> }).state
      : parsed) as { theme?: string; themeId?: string } | null;

    if (!legacy || (!legacy.theme && !legacy.themeId)) {
      return null;
    }

    // Old format: { theme: 'light'|'dark'|'system', themeId: 'spirit' }
    const mode: ThemeMode =
      legacy.theme === 'system' ? getSystemTheme() : legacy.theme === 'dark' ? 'dark' : 'light';
    const oldThemeId = legacy.themeId || 'spirit';

    // Construct new theme ID if needed
    const newThemeId =
      oldThemeId.endsWith('-light') || oldThemeId.endsWith('-dark')
        ? oldThemeId
        : `${oldThemeId}-${mode}`;

    console.log(`Migrating theme from old format: ${oldThemeId} (${legacy.theme}) -> ${newThemeId}`);

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
            const inferredMode: ThemeMode = inferMode(themeId, get().currentMode);
            let tokens: Record<string, string> | undefined;
            if (!themeId.startsWith('spirit-')) {
              tokens = await api.theme.getTokens(themeId);
            }
            applyTheme(themeId, inferredMode, tokens);
            set({ currentThemeId: themeId, currentMode: inferredMode, currentTokens: tokens });
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
          const fallbackMode: ThemeMode = inferMode(themeId, 'light');
          const fallback = fallbackMode === 'dark' ? DEFAULT_THEME_DARK : DEFAULT_THEME_LIGHT;
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
            const fallback = DEFAULT_THEME_LIGHT;
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
            try {
              localStorage.setItem(
                'spiritstream-theme',
                JSON.stringify({ state: { currentThemeId: migrated.themeId }, version: 0 })
              );
            } catch (e) {
              console.error('Failed to update theme storage after migration:', e);
            }
          }

          // Load themes and apply current theme
          state.refreshThemes().then(() => {
            const { currentThemeId } = state;
            state.setTheme(currentThemeId);
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
      const fallback = DEFAULT_THEME_LIGHT;
      state.setTheme(fallback);
    }
  }).catch((error) => {
    console.error('Failed to listen for theme updates:', error);
  });
}
