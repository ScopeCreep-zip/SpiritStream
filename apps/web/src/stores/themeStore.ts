import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { api, events } from '@/lib/backend';
import type { ThemeSummary, ThemeMode } from '@/types/theme';

interface ThemeState {
  currentThemeId: string;
  themes: ThemeSummary[];
  currentTokens?: Record<string, string>;
  isInitialized: boolean;

  // Computed property
  currentMode: ThemeMode;

  // Actions
  setTheme: (themeId: string) => Promise<void>;
  refreshThemes: () => Promise<void>;
  waitForInit: () => Promise<void>;
}

const DEFAULT_THEME_LIGHT = 'spirit-light';
const DEFAULT_THEME_DARK = 'spirit-dark';
const THEME_STYLE_ID = 'spiritstream-theme-overrides';

// Promise-based initialization tracking
let initResolve: (() => void) | null = null;
const initPromise = new Promise<void>((resolve) => {
  initResolve = resolve;
});

function applyTheme(themeId: string, mode: ThemeMode, tokens?: Record<string, string>) {
  if (typeof document === 'undefined') return;

  document.documentElement.setAttribute('data-theme', mode);
  document.documentElement.setAttribute('data-theme-id', themeId);

  // Only apply overrides when tokens are provided and non-empty
  const hasTokens = tokens && Object.keys(tokens).length > 0;
  if (hasTokens) {
    setThemeOverrides(themeId, mode, tokens);
  } else {
    console.warn('[THEME] No tokens provided or empty tokens for theme:', themeId);
    clearThemeOverrides();
  }
}

function setThemeOverrides(themeId: string, mode: ThemeMode, tokens: Record<string, string>) {
  if (typeof document === 'undefined') return;

  const tokenKeys = Object.keys(tokens);
  console.log('[THEME] setThemeOverrides:', {
    themeId,
    mode,
    tokenCount: tokenKeys.length,
    hasPrimary: '--primary' in tokens,
    primaryValue: tokens['--primary'],
  });

  if (tokenKeys.length === 0) {
    console.warn('[THEME] setThemeOverrides called with empty tokens object!');
    return;
  }

  const styleId = THEME_STYLE_ID;
  let style = document.getElementById(styleId) as HTMLStyleElement | null;
  if (!style) {
    style = document.createElement('style');
    style.id = styleId;
    document.head.appendChild(style);
    console.log('[THEME] Created new style element');
  }

  const entries = Object.entries(tokens)
    .map(([key, value]) => `  ${key}: ${value};`)
    .join('\n');

  // Single selector for this specific theme + mode
  const css = `:root[data-theme-id="${themeId}"][data-theme="${mode}"] {\n${entries}\n}`;
  style.textContent = css;
  console.log('[THEME] Injected CSS:', {
    length: css.length,
    selector: `:root[data-theme-id="${themeId}"][data-theme="${mode}"]`,
    sampleCSS: css.slice(0, 200) + '...',
  });

  // DOM verification for debugging
  setTimeout(() => {
    console.log('[THEME] DOM verification:', {
      dataTheme: document.documentElement.getAttribute('data-theme'),
      dataThemeId: document.documentElement.getAttribute('data-theme-id'),
      styleElement: !!document.getElementById(THEME_STYLE_ID),
      computedPrimary: getComputedStyle(document.documentElement).getPropertyValue('--primary').trim(),
      computedBg: getComputedStyle(document.documentElement).getPropertyValue('--bg-base').trim(),
    });
  }, 100);
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

    if (!legacy || !legacy.themeId) {
      return null;
    }

    const themeId = legacy.themeId;
    console.log(`Migrating theme from old format: ${themeId}`);

    return { themeId };
  } catch (error) {
    console.error('Failed to migrate old theme format:', error);
    return null;
  }
}

export const useThemeStore = create<ThemeState>()(
  persist(
    (set, get) => ({
      currentThemeId: DEFAULT_THEME_DARK,
      themes: [],
      currentTokens: undefined,
      currentMode: 'dark',
      isInitialized: false,

      waitForInit: async () => {
        if (get().isInitialized) return;
        await initPromise;
      },

      setTheme: async (themeId) => {
        console.log('[THEME] setTheme called:', themeId);

        try {
          // Wait for themes to be loaded if not initialized (with timeout)
          if (!get().isInitialized) {
            console.log('[THEME] Waiting for themes to load...');
            const timeout = new Promise<void>((_, reject) =>
              setTimeout(() => reject(new Error('Theme initialization timeout')), 10000)
            );
            try {
              await Promise.race([initPromise, timeout]);
            } catch (timeoutError) {
              console.error('[THEME] Initialization timeout, forcing refresh...');
              await get().refreshThemes();
            }
          }

          // Re-get themes AFTER await to ensure we have fresh data
          let { themes } = get();
          console.log('[THEME] themes array:', themes.length, themes.map((t) => t.id));

          let theme = themes.find((t) => t.id === themeId);
          if (!theme) {
            console.warn(`[THEME] Theme ${themeId} not found, attempting refresh...`);
            // Retry: refresh themes and try again
            await get().refreshThemes();
            themes = get().themes;
            theme = themes.find((t) => t.id === themeId);

            if (!theme) {
              console.error(`[THEME] Theme ${themeId} still not found after refresh, falling back to default`);
              const fallback = DEFAULT_THEME_DARK;
              applyTheme(fallback, 'dark', undefined);
              set({ currentThemeId: fallback, currentMode: 'dark', currentTokens: undefined });
              return;
            }
          }

          console.log('[THEME] Found theme:', theme);

          // Check if we already have cached tokens for this theme
          const cachedTokens = get().currentTokens;
          const currentId = get().currentThemeId;
          const hasCachedTokens = currentId === themeId && cachedTokens && Object.keys(cachedTokens).length > 0;

          // Use cached tokens if available (prevents flash), otherwise fetch from backend
          let tokens: Record<string, string> | undefined;
          if (hasCachedTokens) {
            console.log('[THEME] Using cached tokens for theme:', themeId, 'tokenCount:', Object.keys(cachedTokens).length);
            tokens = cachedTokens;
          } else {
            try {
              console.log('[THEME] Fetching tokens for theme:', themeId, 'source:', theme.source);
              tokens = await api.theme.getTokens(themeId);
              // Detailed response logging for debugging
              console.log('[THEME] getTokens response:', {
                type: typeof tokens,
                isNull: tokens === null,
                isUndefined: tokens === undefined,
                keys: tokens ? Object.keys(tokens).length : 0,
                sample: tokens ? Object.entries(tokens).slice(0, 3) : [],
              });

              // Retry once with delay if tokens are empty (helps with timing issues in production)
              if (!tokens || Object.keys(tokens).length === 0) {
                console.warn('[THEME] Empty tokens received, retrying after delay...');
                await new Promise((r) => setTimeout(r, 500));
                tokens = await api.theme.getTokens(themeId);
                console.log('[THEME] Retry getTokens response:', {
                  keys: tokens ? Object.keys(tokens).length : 0,
                });
              }
            } catch (tokenError) {
              console.warn('[THEME] Failed to fetch tokens for theme:', themeId, tokenError);
              // tokens remains undefined - will use CSS defaults from tokens.css
            }
          }

          applyTheme(themeId, theme.mode, tokens);
          set({ currentThemeId: themeId, currentMode: theme.mode, currentTokens: tokens });
          console.log('[THEME] Theme applied successfully:', themeId);
        } catch (error) {
          console.error('[THEME] Failed to load theme:', error);
          // Fall back to default
          const fallback = DEFAULT_THEME_DARK;
          applyTheme(fallback, 'dark', undefined);
          set({ currentThemeId: fallback, currentMode: 'dark', currentTokens: undefined });
        }
      },

      refreshThemes: async () => {
        console.log('[THEME] refreshThemes called');
        try {
          const themes = await api.theme.list();
          console.log('[THEME] Loaded themes:', themes.length, themes.map((t) => t.id));
          set({ themes, isInitialized: true });

          // Resolve the init promise so any waiting setTheme calls can proceed
          if (initResolve) {
            initResolve();
            initResolve = null;
          }

          // Check if current theme still exists
          const { currentThemeId, currentTokens } = get();
          if (!themes.find((theme) => theme.id === currentThemeId)) {
            // If we have cached tokens, trust them - theme data is valid even if not in list
            if (currentTokens && Object.keys(currentTokens).length > 0) {
              console.log('[THEME] Theme not in list but have cached tokens, keeping current theme:', currentThemeId);
            } else {
              console.log('[THEME] Current theme no longer exists and no cached tokens, falling back to default');
              const fallback = DEFAULT_THEME_DARK;
              await get().setTheme(fallback);
            }
          }
        } catch (error) {
          console.error('[THEME] Failed to refresh themes:', error);
          const { currentThemeId, currentTokens } = get();
          // Include current theme in fallback list if we have cached tokens for it
          const fallbackThemes: ThemeSummary[] = [
            { id: DEFAULT_THEME_DARK, name: 'Spirit Dark', mode: 'dark', source: 'builtin' },
            { id: DEFAULT_THEME_LIGHT, name: 'Spirit Light', mode: 'light', source: 'builtin' },
          ];
          // Add current theme to list if it has cached tokens (so it won't be considered "missing")
          if (currentTokens && Object.keys(currentTokens).length > 0 &&
              !fallbackThemes.find(t => t.id === currentThemeId)) {
            const mode: ThemeMode = currentThemeId.includes('-light') ? 'light' : 'dark';
            fallbackThemes.push({
              id: currentThemeId,
              name: currentThemeId,
              mode,
              source: 'builtin', // Use builtin since it's from embedded themes
            });
          }
          set({
            themes: fallbackThemes,
            isInitialized: true,
          });
          // Resolve even on error so we don't block forever
          if (initResolve) {
            initResolve();
            initResolve = null;
          }
        }
      },
    }),
    {
      name: 'spiritstream-theme',
      partialize: (state) => ({
        currentThemeId: state.currentThemeId,
        // Also persist tokens so they can be applied instantly on page load
        currentTokens: state.currentTokens,
      }),
      onRehydrateStorage: () => (state) => {
        if (state) {
          console.log('[THEME] onRehydrateStorage - currentThemeId from localStorage:', state.currentThemeId);
          console.log('[THEME] onRehydrateStorage - cached tokens:', state.currentTokens ? Object.keys(state.currentTokens).length : 0);

          // Try to migrate old format
          const migrated = migrateOldThemeFormat();
          if (migrated) {
            state.currentThemeId = migrated.themeId;
            console.log('[THEME] Migrated theme from old format:', migrated.themeId);
            try {
              localStorage.setItem(
                'spiritstream-theme',
                JSON.stringify({ state: { currentThemeId: migrated.themeId, currentTokens: state.currentTokens }, version: 0 })
              );
            } catch (e) {
              console.error('Failed to update theme storage after migration:', e);
            }
          }

          // Apply cached tokens immediately to prevent flash
          // This ensures React-side tokens match what inline script applied
          if (state.currentTokens && Object.keys(state.currentTokens).length > 0) {
            const mode = state.currentThemeId.includes('-light') ? 'light' : 'dark';
            console.log('[THEME] Applying cached tokens on rehydrate');
            applyTheme(state.currentThemeId, mode as ThemeMode, state.currentTokens);
          }

          // Load themes list - setTheme will be called by useInitialize
          // which gets the authoritative themeId from backend settings
          state.refreshThemes();
        }
      },
    }
  )
);

if (typeof window !== 'undefined') {
  // Listen for theme file changes from backend
  events.on<ThemeSummary[]>('themes_updated', (payload) => {
    const themes = payload;
    useThemeStore.setState({ themes });
    const state = useThemeStore.getState();
    if (!themes.find((theme) => theme.id === state.currentThemeId)) {
      const fallback = DEFAULT_THEME_DARK;
      state.setTheme(fallback);
    }
  }).catch((error) => {
    console.error('Failed to listen for theme updates:', error);
  });
}
