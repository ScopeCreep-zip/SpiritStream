import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronDown, Copy } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import type { OAuthProviderSummary, OAuthConsoleField } from '@spiritstream/api-client';
import { api } from '@/lib/client';
import { toast } from '@/hooks/useToast';
import { logger } from '@/lib/logger';
import { cn } from '@/lib/cn';

interface ProviderCredentialsFormProps {
  summary: OAuthProviderSummary;
  /** Called after a successful save so the parent refreshes summaries. */
  onSaved: (updated: OAuthProviderSummary[]) => void;
}

async function copyValue(value: string, label: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(value);
    toast.success(label);
  } catch {
    toast.error('Copy failed');
  }
}

/**
 * In-app OAuth credential setup. The user registers an app on the
 * provider's developer portal — the one step that can't live in
 * SpiritStream — but they never have to figure out *what to type*: the
 * backend pre-computes every value (the app name derived from their
 * channel, the redirect URL, the category, the client type), and this
 * form shows them as labeled rows with copy buttons. The user pastes
 * those into the portal, then pastes the resulting Client ID back here.
 *
 * Pure presentation: which steps, fields, and values to show all come
 * from `summary.setup`. External URLs are copy-to-clipboard rather than
 * clickable links because the desktop webview denies `shell:open` (it
 * renders chat from strangers).
 */
export function ProviderCredentialsForm({
  summary,
  onSaved,
}: ProviderCredentialsFormProps): React.ReactElement {
  const { t } = useTranslation();
  // Upfront for unconfigured providers; collapsed when this is just the
  // edit-an-existing-credential affordance under a working sign-in.
  const [open, setOpen] = useState(!summary.configured);
  const [clientId, setClientId] = useState(summary.overrideClientId ?? '');
  const [clientSecret, setClientSecret] = useState('');
  const [saving, setSaving] = useState(false);

  const copiedLabel = t('common.copied', { defaultValue: 'Copied' });

  const handleSave = async (): Promise<void> => {
    setSaving(true);
    try {
      const updated = await api.oauth.setProviderCredentials(summary.provider, {
        clientId,
        clientSecret: summary.needsSecret ? clientSecret : undefined,
      });
      setClientSecret('');
      toast.success(
        t('chat.oauth.credentialsSaved', { defaultValue: 'Sign-in credentials saved.' })
      );
      onSaved(updated);
    } catch (error) {
      logger.error(`[ProviderCredentialsForm] ${summary.provider} save failed:`, error);
      toast.error(
        t('chat.oauth.credentialsSaveFailed', {
          defaultValue: 'Could not save credentials: {{error}}',
          error: error instanceof Error ? error.message : String(error),
        })
      );
    } finally {
      setSaving(false);
    }
  };

  const canSave =
    clientId.trim().length > 0 &&
    (!summary.needsSecret || clientSecret.trim().length > 0) &&
    !saving;

  const renderConsoleField = (cf: OAuthConsoleField, index: number): React.ReactElement => (
    <div key={`${cf.label}-${index}`} className="rounded-md border border-border-subtle bg-bg-base p-2">
      <div className="flex items-center justify-between gap-2">
        <span className="text-xs text-text-tertiary">{cf.label}</span>
        {cf.copyable && (
          <Button
            variant="ghost"
            size="sm"
            onClick={() => copyValue(cf.value, copiedLabel)}
            aria-label={t('chat.oauth.copyValue', {
              defaultValue: 'Copy {{label}}',
              label: cf.label,
            })}
          >
            <Copy className="w-3.5 h-3.5" />
          </Button>
        )}
      </div>
      <code className="block text-sm text-text-primary break-all select-all">{cf.value}</code>
      {cf.note && <p className="mt-1 text-xs text-text-tertiary">{cf.note}</p>}
    </div>
  );

  return (
    <div className="mt-2 rounded-md border border-border-subtle bg-bg-sunken">
      <button
        type="button"
        onClick={() => setOpen((prev) => !prev)}
        aria-expanded={open}
        className="w-full flex items-center justify-between gap-2 px-3 py-2 text-sm text-text-primary"
      >
        {t('chat.oauth.setup', { defaultValue: 'Set up sign-in' })}
        <ChevronDown
          className={cn('w-4 h-4 text-text-tertiary transition-transform', open && 'rotate-180')}
        />
      </button>
      {open && (
        <div className="px-3 pb-3 flex flex-col gap-3">
          <p className="text-xs text-text-secondary">
            {t('chat.oauth.setupHint', {
              defaultValue:
                'One-time setup. Open the developer site, create a (free) app using the values below, then paste the Client ID back here. Stored encrypted on this device.',
            })}
          </p>

          {/* Developer portal — copy-to-open (the webview can't launch
              external links; chat-from-strangers threat model). */}
          {renderConsoleField(
            {
              label: t('chat.oauth.portalLabel', { defaultValue: 'Developer portal (open in browser)' }),
              value: summary.registrationUrl,
              copyable: true,
              note: null,
            },
            -1
          )}

          {/* Backend-provided ordered steps (e.g. "Enable YouTube Data API v3"). */}
          {summary.setup.steps.length > 0 && (
            <ol className="list-decimal list-inside flex flex-col gap-1 text-xs text-text-secondary">
              {summary.setup.steps.map((step, i) => (
                <li key={i}>{step}</li>
              ))}
            </ol>
          )}

          {/* Pre-filled console fields the user pastes/selects. */}
          {summary.setup.consoleFields.length > 0 && (
            <div className="flex flex-col gap-2">
              <p className="text-xs font-medium text-text-secondary">
                {t('chat.oauth.pasteTheseValues', {
                  defaultValue: 'Use these values in the app form:',
                })}
              </p>
              {summary.setup.consoleFields.map(renderConsoleField)}
            </div>
          )}

          {/* Paste-back: the credentials the new app gives you. */}
          <div className="border-t border-border-subtle pt-3 flex flex-col gap-3">
            <Input
              label={t('chat.oauth.clientId', { defaultValue: 'Client ID' })}
              value={clientId}
              onChange={(e) => setClientId(e.target.value)}
              autoComplete="off"
            />
            {summary.needsSecret && (
              <Input
                label={t('chat.oauth.clientSecret', { defaultValue: 'Client secret' })}
                value={clientSecret}
                onChange={(e) => setClientSecret(e.target.value)}
                type="password"
                autoComplete="off"
              />
            )}
            <div>
              <Button variant="primary" size="sm" onClick={handleSave} disabled={!canSave}>
                {saving
                  ? t('chat.oauth.saving', { defaultValue: 'Saving…' })
                  : t('chat.oauth.saveCredentials', { defaultValue: 'Save and enable sign-in' })}
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
