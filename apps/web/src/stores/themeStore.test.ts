import { describe, it, expect, beforeEach, vi } from 'vitest';
import type { ThemeSummary } from '@spiritstream/types';

const SPIRIT_DARK = 'spirit-dark';
const SPIRIT_LIGHT = 'spirit-light';
const LIGHT = 'light' as const;

// N5: theme store. Load-bearing behaviour: `refreshThemes` always keeps
// the two bundled themes present (so the picker is never empty when the
// backend is down), `setTheme` writes the `data-theme` attribute + token
// custom properties that the whole UI's colours depend on, and an
// uninstalled-theme request fails loud (toast) instead of silently
// applying nothing — never a silent fallback.

const { api } = vi.hoisted(() => ({
  api: { theme: { list: vi.fn(), getTokens: vi.fn() } },
}));
vi.mock('@/lib/client', () => ({ api }));
vi.mock('@spiritstream/api-client', () => ({ events: { on: vi.fn() } }));
vi.mock('@/lib/constants', () => ({
  clientConfig: { THEME_INIT_TIMEOUT_MS: 50, THEME_TOKEN_RETRY_DELAY_MS: 5 },
}));
const { toast } = vi.hoisted(() => ({ toast: { error: vi.fn(), info: vi.fn() } }));
vi.mock('@/hooks/useToast', () => ({ toast }));
vi.mock('@/lib/logger', () => ({ logger: { error: vi.fn(), warn: vi.fn() } }));

import { useThemeStore } from './themeStore';

function summary(id: string, mode: 'dark' | 'light'): ThemeSummary {
  return {
    id,
    name: id,
    mode,
    source: 'custom',
    builtIn: false,
    valid: true,
    error: null,
  } as unknown as ThemeSummary;
}

beforeEach(() => {
  vi.clearAllMocks();
  useThemeStore.setState({
    currentThemeId: SPIRIT_DARK,
    currentMode: 'dark',
    currentTokens: undefined,
    isInitialized: true,
    themes: [summary(SPIRIT_DARK, 'dark'), summary(SPIRIT_LIGHT, LIGHT)],
  });
  document.documentElement.removeAttribute('data-theme');
  document.documentElement.removeAttribute('data-theme-id');
});

describe('themeStore.refreshThemes', () => {
  it('merges backend themes while always keeping the bundled two', async () => {
    api.theme.list.mockResolvedValue([summary('custom-neon', 'dark')]);
    await useThemeStore.getState().refreshThemes();
    const ids = useThemeStore.getState().themes.map((t) => t.id);
    expect(ids).toContain('custom-neon');
    expect(ids).toContain(SPIRIT_DARK);
    expect(ids).toContain(SPIRIT_LIGHT);
    expect(useThemeStore.getState().isInitialized).toBe(true);
  });

  it('keeps existing themes and toasts on backend failure', async () => {
    api.theme.list.mockRejectedValue(new Error('backend down'));
    await expect(useThemeStore.getState().refreshThemes()).rejects.toThrow('backend down');
    expect(toast.error).toHaveBeenCalled();
    // Bundled themes survive a failed refresh.
    expect(useThemeStore.getState().themes.length).toBeGreaterThanOrEqual(2);
  });
});

describe('themeStore.setTheme', () => {
  it('applies the data-theme attribute and caches tokens', async () => {
    api.theme.getTokens.mockResolvedValue({ '--bg-base': '#ffffff' });
    await useThemeStore.getState().setTheme(SPIRIT_LIGHT);
    expect(document.documentElement.getAttribute('data-theme')).toBe(LIGHT);
    expect(document.documentElement.getAttribute('data-theme-id')).toBe(SPIRIT_LIGHT);
    const s = useThemeStore.getState();
    expect(s.currentThemeId).toBe(SPIRIT_LIGHT);
    expect(s.currentMode).toBe(LIGHT);
    expect(s.currentTokens).toEqual({ '--bg-base': '#ffffff' });
  });

  it('short-circuits when the theme is already current with cached tokens', async () => {
    useThemeStore.setState({
      currentThemeId: SPIRIT_DARK,
      currentTokens: { '--bg-base': '#000' },
    });
    await useThemeStore.getState().setTheme(SPIRIT_DARK);
    expect(api.theme.getTokens).not.toHaveBeenCalled();
  });

  it('fails loud (toast + throw) when the theme is not installed', async () => {
    api.theme.list.mockResolvedValue([summary(SPIRIT_DARK, 'dark')]);
    await expect(useThemeStore.getState().setTheme('ghost-theme')).rejects.toThrow(/not installed/);
    expect(toast.error).toHaveBeenCalledWith(expect.stringContaining('not installed'));
  });

  it('still applies the theme when token fetch fails (CSS defaults take over)', async () => {
    api.theme.getTokens.mockRejectedValue(new Error('no tokens'));
    await useThemeStore.getState().setTheme(SPIRIT_LIGHT);
    // Attribute still flips; tokens stay undefined so tokens.css defaults apply.
    expect(document.documentElement.getAttribute('data-theme')).toBe(LIGHT);
    expect(useThemeStore.getState().currentThemeId).toBe(SPIRIT_LIGHT);
  });
});

describe('themeStore.waitForInit', () => {
  it('resolves immediately once initialized', async () => {
    useThemeStore.setState({ isInitialized: true });
    await expect(useThemeStore.getState().waitForInit()).resolves.toBeUndefined();
  });
});
