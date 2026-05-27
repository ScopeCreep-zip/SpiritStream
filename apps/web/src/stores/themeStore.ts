import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import { api } from '@/lib/client';
import { events } from '@spiritstream/api-client';
import { clientConfig } from '@/lib/constants';
import { toast } from '@/hooks/useToast';
import { logger } from '@/lib/logger';
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

const DEFAULT_THEME_DARK = 'spirit-dark';
const THEME_STYLE_ID = 'spiritstream-theme-overrides';

// Promise-based initialization tracking
let initResolve: (() => void) | null = null;
const initPromise = new Promise<void>((resolve) => {
  initResolve = resolve;
});

// Bundled themes are always present in the catalog because their tokens live
// in `tokens.css`. The store starts with these and merges backend additions on
// refresh, so the picker is never empty even when the backend is unreachable.
const BUNDLED_THEMES: ReadonlyArray<ThemeSummary> = [
  { id: 'spirit-dark', name: 'Spirit Dark', mode: 'dark', source: 'builtin', builtIn: true, valid: true, error: null },
  { id: 'spirit-light', name: 'Spirit Light', mode: 'light', source: 'builtin', builtIn: true, valid: true, error: null },
];

/**
 * Apply a theme. Owns ONLY `data-theme`, `data-theme-id`, and the
 * `<style id="spiritstream-theme-overrides">` token tag. The `data-contrast`
 * attribute is the exclusive domain of `useHighContrast` — never touch it here.
 * Theme and contrast are orthogonal axes; they must not read or write each
 * other's attributes.
 */
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
      // Start with the bundled themes — they're always installed (tokens live
      // in tokens.css). Backend additions get merged in on refreshThemes().
      themes: [...BUNDLED_THEMES],
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
              const err = new Error(`Theme "${themeId}" is not installed.`);
              toast.error(err.message);
              throw err;
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
        } catch (err) {
          logger.error('[themeStore] setTheme failed', err);
          // Surface unless we already toasted from the not-installed branch.
          const message = err instanceof Error ? err.message : String(err);
          if (!message.startsWith('Theme "')) {
            toast.error(`Failed to load theme: ${message}`);
          }
          throw err;
        }
      },

      refreshThemes: async () => {
        try {
          const fromBackend = await api.theme.list();
          // Merge backend themes with the always-present bundled ones.
          // Backend entries take precedence on id collision so any backend-
          // overridden metadata wins.
          const backendIds = new Set(fromBackend.map((t) => t.id));
          const merged: ThemeSummary[] = [
            ...fromBackend,
            ...BUNDLED_THEMES.filter((t) => !backendIds.has(t.id)),
          ];
          set({ themes: merged, isInitialized: true });

          if (initResolve) {
            initResolve();
            initResolve = null;
          }

          // If the user's current theme was uninstalled, switch to the default
          // and tell them — never silently swap.
          const { currentThemeId, currentTokens } = get();
          if (!merged.find((theme) => theme.id === currentThemeId)) {
            // Cached tokens still let the current theme render; trust them.
            const hasCached = currentTokens && Object.keys(currentTokens).length > 0;
            if (!hasCached) {
              toast.info(
                `Theme "${currentThemeId}" was uninstalled. Switched to Spirit Dark.`,
              );
              await get().setTheme(DEFAULT_THEME_DARK);
            }
          }
        } catch (err) {
          logger.error('[themeStore] refreshThemes failed', err);
          toast.error(
            `Could not load themes from backend: ${err instanceof Error ? err.message : String(err)}`,
          );
          // Mark as initialized so callers don't block forever; existing
          // `themes` state (at minimum the bundled themes from initial state)
          // remains untouched — no manufactured catalog.
          set({ isInitialized: true });
          if (initResolve) {
            initResolve();
            initResolve = null;
          }
          throw err;
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
    const backendIds = new Set(payload.map((t) => t.id));
    const merged: ThemeSummary[] = [
      ...payload,
      ...BUNDLED_THEMES.filter((t) => !backendIds.has(t.id)),
    ];
    useThemeStore.setState({ themes: merged });
    const state = useThemeStore.getState();
    if (!merged.find((theme) => theme.id === state.currentThemeId)) {
      toast.info(
        `Theme "${state.currentThemeId}" was uninstalled. Switched to Spirit Dark.`,
      );
      // Fire-and-forget — caller is an event subscription, can't await.
      state.setTheme(DEFAULT_THEME_DARK).catch(() => {
        /* swallow: toast already surfaced the uninstall */
      });
    }
  });
}
