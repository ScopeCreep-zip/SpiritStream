import { fetchTypedJson } from './_internal';

/**
 * HMAC chain verification status returned alongside every audit log
 * fetch. The audit-log UI inspects `chain.state` on each load and
 * renders the red tamper banner when it equals `"tampered"`.
 */
export type AuditChainStatus =
  | { state: 'ok'; entriesVerified: number }
  | { state: 'tampered'; lastValidSequence: number; reason: string }
  | { state: 'empty' };

export interface AuditLogResponse {
  total: number;
  entries: unknown[];
  chain: AuditChainStatus;
}

export const audit = {
  /**
   * Read the audit log. Pagination via `skip` + `limit`; optional
   * `kind` filter (e.g. `"panic_triggered"`, `"oauth_refresh"`).
   * Always returns a `chain` field — the audit-log UI must read it
   * on every fetch to render the tamper banner.
   */
  log: (opts?: { skip?: number; limit?: number; kind?: string }) => {
    const params: Record<string, string> = {};
    if (opts?.skip !== undefined) params.skip = String(opts.skip);
    if (opts?.limit !== undefined) params.limit = String(opts.limit);
    if (opts?.kind !== undefined) params.kind = opts.kind;
    return fetchTypedJson<AuditLogResponse>(
      'GET',
      '/api/v1/audit/log',
      Object.keys(params).length > 0 ? params : undefined
    );
  },
};
