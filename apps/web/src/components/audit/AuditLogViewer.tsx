import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { api } from '@/lib/client';
import type { AuditChainStatus } from '@spiritstream/api-client';
import { formatDateTime } from '@/lib/locale';

/**
 * Audit log view.
 *
 * Reads from `GET /api/v1/audit/log`. Paginated + filterable by event
 * kind. Renders a red tamper banner when the server-computed HMAC
 * chain status returns `tampered` — the verification runs on every
 * fetch (see `AuditLogService::verify_chain`).
 */
interface AuditEntry {
  timestamp: string;
  action: { kind: string } & Record<string, unknown>;
  detail?: string | null;
}

const PAGE_SIZE = 100;

const KIND_FILTERS: { value: string; labelKey: string }[] = [
  { value: '', labelKey: 'audit.filterAll' },
  { value: 'panic_triggered', labelKey: 'audit.filterPanic' },
  { value: 'chat_message_pii_blocked', labelKey: 'audit.filterPii' },
  { value: 'oauth_refresh', labelKey: 'audit.filterOauthRefresh' },
  { value: 'oauth_refresh_unusual_location', labelKey: 'audit.filterOauthUnusual' },
  { value: 'profile_saved', labelKey: 'audit.filterProfileSaved' },
  { value: 'profile_deleted', labelKey: 'audit.filterProfileDeleted' },
  { value: 'machine_key_rotated', labelKey: 'audit.filterMachineKey' },
];

export function AuditLogViewer(): React.ReactElement {
  const { t } = useTranslation();
  const [entries, setEntries] = useState<AuditEntry[]>([]);
  const [total, setTotal] = useState(0);
  const [skip, setSkip] = useState(0);
  const [kind, setKind] = useState('');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [chain, setChain] = useState<AuditChainStatus | null>(null);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    api.audit
      .log({ skip, limit: PAGE_SIZE, kind: kind || undefined })
      .then((res) => {
        if (cancelled) return;
        setEntries(res.entries as AuditEntry[]);
        setTotal(res.total);
        setChain(res.chain);
      })
      .catch((e: Error) => {
        if (cancelled) return;
        setError(e.message);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [skip, kind]);

  return (
    <div className="flex flex-col gap-4 p-6">
      <header className="flex flex-wrap items-baseline justify-between gap-3">
        <h1 className="text-2xl font-semibold text-text-primary">
          {t('audit.title', 'Audit log')}
        </h1>
        <p className="text-sm text-text-tertiary">
          {t('audit.subtitle', 'Every security-relevant event SpiritStream has recorded.')}
        </p>
      </header>

      {/* Tamper banner — driven by the server-computed HMAC chain status.
          `audit-tamper-slot` is the historical hook the test suite uses to
          assert the banner is reachable; keep the data-testid on the always-
          rendered wrapper so it survives the conditional inside. */}
      <div data-testid="audit-tamper-slot">
        {chain?.state === 'tampered' && (
          <div
            role="alert"
            className="bg-error-subtle border border-error-border rounded-lg p-4"
          >
            <h3 className="text-error-text font-semibold">
              {t('audit.tampered.title', 'Audit log tampered')}
            </h3>
            <p className="text-text-secondary text-sm mt-1">
              {t('audit.tampered.message', {
                defaultValue:
                  'HMAC chain verification failed at sequence {{lastValid}}. Entries before this point can still be trusted; entries after may have been modified, inserted, or deleted out of band. Reason: {{reason}}.',
                lastValid: chain.lastValidSequence,
                reason: chain.reason,
              })}
            </p>
          </div>
        )}
      </div>

      <div className="flex flex-wrap gap-2 items-center">
        <label className="text-sm text-text-secondary" htmlFor="audit-kind">
          {t('audit.filterLabel', 'Filter')}
        </label>
        <select
          id="audit-kind"
          value={kind}
          onChange={(e) => {
            setSkip(0);
            setKind(e.target.value);
          }}
          className="text-sm px-2 py-1 rounded border border-border-default bg-bg-surface"
        >
          {KIND_FILTERS.map((opt) => (
            <option key={opt.value} value={opt.value}>
              {t(opt.labelKey, opt.value || 'All')}
            </option>
          ))}
        </select>
        <span className="ms-auto text-xs text-text-tertiary">
          {t('audit.total', '{{count}} entries', { count: total })}
        </span>
      </div>

      {error && (
        <div className="p-3 bg-error-subtle border border-error-border rounded-lg text-sm text-error-text">
          {error}
        </div>
      )}

      <div className="rounded-lg border border-border-default overflow-hidden">
        <table className="w-full text-sm">
          <thead className="bg-bg-muted text-text-secondary">
            <tr>
              <th className="text-left px-3 py-2 font-medium">
                {t('audit.colTimestamp', 'Time')}
              </th>
              <th className="text-left px-3 py-2 font-medium">
                {t('audit.colKind', 'Event')}
              </th>
              <th className="text-left px-3 py-2 font-medium">
                {t('audit.colDetails', 'Details')}
              </th>
            </tr>
          </thead>
          <tbody>
            {(() => {
              if (loading && entries.length === 0) {
                return (
                  <tr>
                    <td colSpan={3} className="px-3 py-4 text-text-tertiary text-center">
                      {t('audit.loading', 'Loading…')}
                    </td>
                  </tr>
                );
              }
              if (entries.length === 0) {
                return (
                  <tr>
                    <td colSpan={3} className="px-3 py-4 text-text-tertiary text-center">
                      {t('audit.empty', 'No audit entries match this filter.')}
                    </td>
                  </tr>
                );
              }
              return entries.map((entry, idx) => (
                <tr
                  key={`${entry.timestamp}-${idx}`}
                  className="border-t border-border-subtle hover:bg-bg-hover"
                >
                  <td className="px-3 py-2 text-text-secondary whitespace-nowrap">
                    {formatDateTime(entry.timestamp)}
                  </td>
                  <td className="px-3 py-2 text-text-primary font-medium">
                    {entry.action.kind}
                  </td>
                  <td className="px-3 py-2 text-text-tertiary">
                    {formatActionDetails(entry.action)}
                  </td>
                </tr>
              ));
            })()}
          </tbody>
        </table>
      </div>

      <footer className="flex items-center justify-between text-sm">
        <button
          type="button"
          onClick={() => setSkip(Math.max(0, skip - PAGE_SIZE))}
          disabled={skip === 0 || loading}
          className="px-3 py-1.5 rounded border border-border-default text-text-primary hover:bg-bg-hover disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {t('audit.prev', '← Previous')}
        </button>
        <span className="text-text-tertiary">
          {t('audit.page', '{{from}}–{{to}} of {{total}}', {
            from: Math.min(skip + 1, total),
            to: Math.min(skip + entries.length, total),
            total,
          })}
        </span>
        <button
          type="button"
          onClick={() => setSkip(skip + PAGE_SIZE)}
          disabled={skip + entries.length >= total || loading}
          className="px-3 py-1.5 rounded border border-border-default text-text-primary hover:bg-bg-hover disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {t('audit.next', 'Next →')}
        </button>
      </footer>
    </div>
  );
}

function formatActionDetails(action: AuditEntry['action']): string {
  // Render a compact one-line view of each action's payload, skipping
  // the discriminator `kind` field we already show in the kind column.
  const entries = Object.entries(action).filter(([k]) => k !== 'kind');
  if (entries.length === 0) return '—';
  return entries.map(([k, v]) => `${k}=${String(v)}`).join(' · ');
}
