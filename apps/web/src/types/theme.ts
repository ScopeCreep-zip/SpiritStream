export type ThemeMode = 'light' | 'dark';

export interface ThemeSummary {
  id: string;
  name: string;
  mode: ThemeMode;
  source: 'builtin' | 'custom';
}

export type ThemeTokens = Record<string, string>;
