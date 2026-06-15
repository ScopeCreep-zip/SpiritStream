import { v1AuditLog } from '../generated';

/**
 * HMAC chain verification status returned alongside every audit log
 * fetch. The audit-log UI inspects `chain.state` on each load and
 * renders the red tamper banner when it equals `"tampered"`.
 *
 * Hand-typed as a discriminated union — richer than the flat object the
 * OpenAPI generator emits — so the UI can exhaustively switch on `state`.
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
  log: async (opts?: {
    skip?: number;
    limit?: number;
    kind?: string;
  }): Promise<AuditLogResponse> => {
    const query: { skip?: number; limit?: number; kind?: string } = {};
    if (opts?.skip !== undefined) query.skip = opts.skip;
    if (opts?.limit !== undefined) query.limit = opts.limit;
    if (opts?.kind !== undefined) query.kind = opts.kind;
    const { data } = await v1AuditLog({
      query: Object.keys(query).length > 0 ? query : undefined,
      throwOnError: true,
    });
    return data as unknown as AuditLogResponse;
  },
};
