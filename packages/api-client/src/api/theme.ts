import type { ThemeSummary } from '@spiritstream/types';
import {
  v1ThemesListProxy,
  v1ThemeTokensProxy,
  v1ThemesInstallProxy,
  v1ThemesRefreshProxy,
} from '../generated';

export const theme = {
  list: async (): Promise<ThemeSummary[]> => {
    const { data } = await v1ThemesListProxy({ throwOnError: true });
    return data as ThemeSummary[];
  },
  getTokens: async (themeId: string): Promise<Record<string, string>> => {
    const { data } = await v1ThemeTokensProxy({ path: { theme_id: themeId }, throwOnError: true });
    return data.tokens;
  },
  install: async (themePath: string): Promise<ThemeSummary> => {
    const { data } = await v1ThemesInstallProxy({ body: { themePath }, throwOnError: true });
    return data as ThemeSummary;
  },
  refresh: async (): Promise<ThemeSummary[]> => {
    const { data } = await v1ThemesRefreshProxy({ throwOnError: true });
    return data as ThemeSummary[];
  },
};
