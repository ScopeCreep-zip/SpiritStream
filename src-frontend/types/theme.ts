export interface ThemeSummary {
  id: string;
  name: string;
  source: 'builtin' | 'custom';
}

export interface ThemeTokens {
  light: Record<string, string>;
  dark: Record<string, string>;
}
