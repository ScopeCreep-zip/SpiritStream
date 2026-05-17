import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { api } from '@/lib/client';
import { events } from '@spiritstream/api-client';
import { clientConfig } from '@/lib/constants';
import type { ThemeSummary, ThemeMode } from '@spiritstream/types';

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
    clearThemeOverrides();
  }
}

function setThemeOverrides(themeId: string, mode: ThemeMode, tokens: Record<string, string>) {
  if (typeof document === 'undefined') return;

  const tokenKeys = Object.keys(tokens);
  if (tokenKeys.length === 0) {
    return;
  }

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
  const css = `:root[data-theme-id="${themeId}"][data-theme="${mode}"] {\n${entries}\n}`;
  style.textContent = css;
}

function clearThemeOverrides() {
  if (typeof document === 'undefined') return;
  const style = document.getElementById(THEME_STYLE_ID);
  if (style && style.parentNode) {
    style.parentNode.removeChild(style);
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
        // Short-circuit when the requested theme is already current AND we
        // have its tokens cached. The cached path is self-sufficient — it
        // does not depend on the catalog being loaded. (Previously gated
        // on `isInitialized`, which made cache-hits block on the network
        // load of the catalog. Fixed alongside the boot-order rework so
        // setTheme works correctly before the readiness gate passes.)
        //
        // loadProfile fires both a synchronous
        // applyProfileSettings call AND consumes the `profile_activated`
        // event, so setTheme can be called twice in quick succession for
        // the same themeId — without this guard the second call burns a
        // backend round-trip on `api.theme.getTokens`.
        {
          const { currentThemeId, currentTokens } = get();
          if (
            currentThemeId === themeId
            && currentTokens
            && Object.keys(currentTokens).length > 0
          ) {
            return;
          }
        }
        try {
          // Wait for themes to be loaded if not initialized (with timeout)
          if (!get().isInitialized) {
            const timeout = new Promise<void>((_, reject) =>
              setTimeout(() => reject(new Error('Theme initialization timeout')), clientConfig.THEME_INIT_TIMEOUT_MS)
            );
            try {
              await Promise.race([initPromise, timeout]);
            } catch {
              await get().refreshThemes();
            }
          }

          // Re-get themes AFTER await to ensure we have fresh data
          let { themes } = get();

          let theme = themes.find((t) => t.id === themeId);
          if (!theme) {
            // Retry: refresh themes and try again
            await get().refreshThemes();
            themes = get().themes;
            theme = themes.find((t) => t.id === themeId);

            if (!theme) {
              const fallback = DEFAULT_THEME_DARK;
              applyTheme(fallback, 'dark', undefined);
              set({ currentThemeId: fallback, currentMode: 'dark', currentTokens: undefined });
              return;
            }
          }

          // Check if we already have cached tokens for this theme
          const cachedTokens = get().currentTokens;
          const currentId = get().currentThemeId;
          const hasCachedTokens = currentId === themeId && cachedTokens && Object.keys(cachedTokens).length > 0;

          // Use cached tokens if available (prevents flash), otherwise fetch from backend
          let tokens: Record<string, string> | undefined;
          if (hasCachedTokens) {
            tokens = cachedTokens;
          } else {
            try {
              tokens = await api.theme.getTokens(themeId);

              // Retry once with delay if tokens are empty (helps with timing issues in production)
              if (!tokens || Object.keys(tokens).length === 0) {
                await new Promise((r) => setTimeout(r, clientConfig.THEME_TOKEN_RETRY_DELAY_MS));
                tokens = await api.theme.getTokens(themeId);
              }
            } catch {
              // tokens remains undefined - will use CSS defaults from tokens.css
            }
          }

          applyTheme(themeId, theme.mode, tokens);
          set({ currentThemeId: themeId, currentMode: theme.mode, currentTokens: tokens });
        } catch {
          // Fall back to default
          const fallback = DEFAULT_THEME_DARK;
          applyTheme(fallback, 'dark', undefined);
          set({ currentThemeId: fallback, currentMode: 'dark', currentTokens: undefined });
        }
      },

      refreshThemes: async () => {
        try {
          const themes = await api.theme.list();
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
            if (!currentTokens || Object.keys(currentTokens).length === 0) {
              const fallback = DEFAULT_THEME_DARK;
              await get().setTheme(fallback);
            }
          }
        } catch {
          const { currentThemeId, currentTokens } = get();
          // Include current theme in fallback list if we have cached tokens for it
          const fallbackThemes: ThemeSummary[] = [
            { id: DEFAULT_THEME_DARK, name: 'Spirit Dark', mode: 'dark', source: 'builtin', builtIn: true, valid: true, error: null },
            { id: DEFAULT_THEME_LIGHT, name: 'Spirit Light', mode: 'light', source: 'builtin', builtIn: true, valid: true, error: null },
          ];
          // Add current theme to list if it has cached tokens (so it won't be considered "missing")
          if (currentTokens && Object.keys(currentTokens).length > 0 &&
              !fallbackThemes.find(t => t.id === currentThemeId)) {
            const mode: ThemeMode = currentThemeId.includes('-light') ? 'light' : 'dark';
            fallbackThemes.push({
              id: currentThemeId,
              name: currentThemeId,
              mode,
              source: 'builtin', // bundled embedded theme
              builtIn: true,
              valid: true,
              error: null,
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
          // Mirror the cached theme into React-side state. The inline
          // <script> in index.html has already set data-theme and the
          // token <style> on <html> before CSS parsed, so this is just
          // a React-side mirror — the DOM is already correct.
          //
          // NO network call here. Per the next-themes / MUI pattern,
          // localStorage is the source of truth for first paint; the
          // server's theme catalog is loaded later (in AppContent,
          // after the readiness gate) where it can fail without
          // blocking the boot path.
          if (state.currentTokens && Object.keys(state.currentTokens).length > 0) {
            const mode = state.currentThemeId.includes('-light') ? 'light' : 'dark';
            applyTheme(state.currentThemeId, mode as ThemeMode, state.currentTokens);
          }
        }
      },
    }
  )
);

/**
 * Subscribe to backend `themes_updated` events. Called by `AppContent`
 * after the server-readiness gate passes — calling at module import
 * raced server startup and opened a WebSocket before the backend was
 * listening. Returns an unsubscribe function for React `useEffect`.
 */
export async function subscribeThemesUpdated(): Promise<() => void> {
  return events.on<ThemeSummary[]>('themes_updated', (payload) => {
    const themes = payload;
    useThemeStore.setState({ themes });
    const state = useThemeStore.getState();
    if (!themes.find((theme) => theme.id === state.currentThemeId)) {
      const fallback = DEFAULT_THEME_DARK;
      state.setTheme(fallback);
    }
  });
}
