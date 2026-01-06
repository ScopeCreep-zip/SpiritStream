import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { listen } from '@tauri-apps/api/event';
import { api } from '@/lib/tauri';
import type { ThemeSummary, ThemeTokens } from '@/types/theme';

export type Theme = 'light' | 'dark' | 'system';

interface ThemeState {
  theme: Theme;
  resolvedTheme: 'light' | 'dark';
  themeId: string;
  themes: ThemeSummary[];
  currentTokens?: ThemeTokens;
  setTheme: (theme: Theme) => void;
  toggleTheme: () => void;
  setThemeId: (themeId: string) => Promise<void>;
  refreshThemes: () => Promise<void>;
}

const BUILTIN_THEME_ID = 'spirit';
const THEME_STYLE_ID = 'spiritstream-theme-overrides';

function getSystemTheme(): 'light' | 'dark' {
  if (typeof window === 'undefined') return 'light';
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

function resolveTheme(theme: Theme): 'light' | 'dark' {
  return theme === 'system' ? getSystemTheme() : theme;
}

function applyThemeAttributes(resolvedTheme: 'light' | 'dark', themeId: string) {
  if (typeof document !== 'undefined') {
    document.documentElement.setAttribute('data-theme', resolvedTheme);
    document.documentElement.setAttribute('data-theme-name', themeId);
  }
}

function buildThemeCss(themeId: string, tokens: ThemeTokens): string {
  const buildBlock = (mode: 'light' | 'dark', entries: Record<string, string>) => {
    const lines = Object.entries(entries)
      .map(([key, value]) => `  ${key}: ${value};`)
      .join('\n');
    return `:root[data-theme-name="${themeId}"][data-theme="${mode}"] {\n${lines}\n}`;
  };

  return `${buildBlock('light', tokens.light)}\n${buildBlock('dark', tokens.dark)}`;
}

function setThemeOverrides(themeId: string, tokens: ThemeTokens) {
  if (typeof document === 'undefined') return;
  const styleId = THEME_STYLE_ID;
  let style = document.getElementById(styleId) as HTMLStyleElement | null;
  if (!style) {
    style = document.createElement('style');
    style.id = styleId;
    document.head.appendChild(style);
  }
  style.textContent = buildThemeCss(themeId, tokens);
}

function clearThemeOverrides() {
  if (typeof document === 'undefined') return;
  const style = document.getElementById(THEME_STYLE_ID);
  if (style && style.parentNode) {
    style.parentNode.removeChild(style);
  }
}

function applyTheme(
  resolvedTheme: 'light' | 'dark',
  themeId: string,
  tokens?: ThemeTokens
) {
  applyThemeAttributes(resolvedTheme, themeId);
  if (themeId === BUILTIN_THEME_ID || !tokens) {
    clearThemeOverrides();
  } else {
    setThemeOverrides(themeId, tokens);
  }
}

export const useThemeStore = create<ThemeState>()(
  persist(
    (set, get) => ({
      theme: 'system',
      resolvedTheme: getSystemTheme(),
      themeId: BUILTIN_THEME_ID,
      themes: [],
      currentTokens: undefined,

      setTheme: (theme) => {
        const resolvedTheme = resolveTheme(theme);
        applyTheme(resolvedTheme, get().themeId, get().currentTokens);
        set({ theme, resolvedTheme });
      },

      toggleTheme: () => {
        const current = get().resolvedTheme;
        const next = current === 'light' ? 'dark' : 'light';
        applyTheme(next, get().themeId, get().currentTokens);
        set({ theme: next, resolvedTheme: next });
      },

      setThemeId: async (themeId) => {
        const resolvedTheme = resolveTheme(get().theme);
        if (themeId === BUILTIN_THEME_ID) {
          applyTheme(resolvedTheme, themeId, undefined);
          set({ themeId, currentTokens: undefined });
          return;
        }

        try {
          const tokens = await api.theme.getTokens(themeId);
          applyTheme(resolvedTheme, themeId, tokens);
          set({ themeId, currentTokens: tokens });
        } catch (error) {
          console.error('Failed to load theme tokens:', error);
          applyTheme(resolvedTheme, BUILTIN_THEME_ID, undefined);
          set({ themeId: BUILTIN_THEME_ID, currentTokens: undefined });
        }
      },

      refreshThemes: async () => {
        try {
          const themes = await api.theme.list();
          set({ themes });
          if (!themes.find((theme) => theme.id === get().themeId)) {
            await get().setThemeId(BUILTIN_THEME_ID);
          }
        } catch (error) {
          console.error('Failed to refresh themes:', error);
          set({ themes: [{ id: BUILTIN_THEME_ID, name: 'Spirit', source: 'builtin' }] });
          await get().setThemeId(BUILTIN_THEME_ID);
        }
      },
    }),
    {
      name: 'spiritstream-theme',
      partialize: (state) => ({
        theme: state.theme,
        themeId: state.themeId,
      }),
      onRehydrateStorage: () => (state) => {
        if (state) {
          const resolvedTheme = resolveTheme(state.theme);
          applyTheme(resolvedTheme, state.themeId, state.currentTokens);
          state.resolvedTheme = resolvedTheme;
          if (state.themeId !== BUILTIN_THEME_ID) {
            state.setThemeId(state.themeId);
          }
        }
      },
    }
  )
);

if (typeof window !== 'undefined') {
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', (e) => {
    const state = useThemeStore.getState();
    if (state.theme === 'system') {
      const resolvedTheme = e.matches ? 'dark' : 'light';
      applyTheme(resolvedTheme, state.themeId, state.currentTokens);
      useThemeStore.setState({ resolvedTheme });
    }
  });

  listen<ThemeSummary[]>('themes_updated', (event) => {
    const themes = event.payload;
    useThemeStore.setState({ themes });
    const state = useThemeStore.getState();
    if (!themes.find((theme) => theme.id === state.themeId)) {
      state.setThemeId(BUILTIN_THEME_ID);
    }
  }).catch((error) => {
    console.error('Failed to listen for theme updates:', error);
  });
}
