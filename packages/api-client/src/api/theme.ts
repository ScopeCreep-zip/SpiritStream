import type { ThemeSummary } from '@spiritstream/types';
import { fetchTypedJson } from './_internal';

export const theme = {
  list: () => fetchTypedJson<ThemeSummary[]>('GET', '/api/v1/themes'),
  getTokens: (themeId: string) =>
    fetchTypedJson<{ tokens: Record<string, string> }>(
      'GET',
      `/api/v1/themes/${encodeURIComponent(themeId)}/tokens`
    ).then((r) => r.tokens),
  install: (themePath: string) =>
    fetchTypedJson<ThemeSummary>('POST', '/api/v1/themes', undefined, { themePath }),
  refresh: () => fetchTypedJson<ThemeSummary[]>('POST', '/api/v1/themes/refresh'),
};
