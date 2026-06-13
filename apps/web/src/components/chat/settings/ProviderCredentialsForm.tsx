import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronDown, ExternalLink } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import type { OAuthProviderSummary } from '@spiritstream/api-client';
import { api } from '@/lib/client';
import { toast } from '@/hooks/useToast';
import { logger } from '@/lib/logger';
import { cn } from '@/lib/cn';

interface ProviderCredentialsFormProps {
  summary: OAuthProviderSummary;
  /** Called after a successful save so the parent refetches summaries. */
  onSaved: (updated: OAuthProviderSummary[]) => void;
}

/**
 * In-app OAuth credential setup — the user registers an app on the
 * provider's developer portal (the one step that can't live here),
 * pastes the credentials, and the backend stores them encrypted so
 * they survive restarts. No env files, no rebuild.
 *
 * Pure presentation: which fields to show (`needsSecret`), where to
 * register (`registrationUrl`), and whether it worked all come from
 * the backend summary.
 */
export function ProviderCredentialsForm({
  summary,
  onSaved,
}: ProviderCredentialsFormProps): React.ReactElement {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [clientId, setClientId] = useState(summary.overrideClientId ?? '');
  const [clientSecret, setClientSecret] = useState('');
  const [saving, setSaving] = useState(false);

  const platformName = t(`chat.platforms.${summary.provider}`);

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
                'One-time setup: create a (free) app on {{platform}}’s developer site, then paste its credentials here. They’re stored encrypted on this device.',
              platform: platformName,
            })}
          </p>
          <a
            href={summary.registrationUrl}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-1.5 text-xs text-primary hover:underline"
          >
            <ExternalLink className="w-3.5 h-3.5" aria-hidden="true" />
            {t('chat.oauth.openPortal', {
              defaultValue: 'Open {{platform}} developer portal',
              platform: platformName,
            })}
          </a>
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
      )}
    </div>
  );
}
