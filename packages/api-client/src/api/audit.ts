import { fetchTypedJson } from './_internal';

export const audit = {
  /**
   * Read the audit log. Pagination via `skip` + `limit`; optional
   * `kind` filter (e.g. `"panic_triggered"`, `"oauth_refresh"`).
   */
  log: (opts?: { skip?: number; limit?: number; kind?: string }) => {
    const params: Record<string, string> = {};
    if (opts?.skip !== undefined) params.skip = String(opts.skip);
    if (opts?.limit !== undefined) params.limit = String(opts.limit);
    if (opts?.kind !== undefined) params.kind = opts.kind;
    return fetchTypedJson<{ total: number; entries: unknown[] }>(
      'GET',
      '/api/v1/audit/log',
      Object.keys(params).length > 0 ? params : undefined,
    );
  },
};
