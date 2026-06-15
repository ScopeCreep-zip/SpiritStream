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

const TOTAL_STEPS = 3;

async function copyValue(value: string, label: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(value);
    toast.success(label);
  } catch {
    toast.error('Copy failed');
  }
}

/**
 * In-app OAuth credential setup, as a 3-step wizard so the (necessarily
 * involved) provider-portal registration never reads as a wall of text:
 *
 *   1. Open the developer site (copy the link — the webview can't launch
 *      external URLs, by the chat-from-strangers threat model).
 *   2. Set up the app there using the backend-computed steps + paste values.
 *   3. Paste the resulting Client ID (and secret, if the provider needs one)
 *      back into SpiritStream.
 *
 * Pure presentation: every step, field, and value comes from `summary.setup`
 * — the frontend holds zero provider knowledge.
 */
export function ProviderCredentialsForm({
  summary,
  onSaved,
}: ProviderCredentialsFormProps): React.ReactElement {
  const { t } = useTranslation();
  // Upfront for unconfigured providers; collapsed when this is the
  // edit-an-existing-credential affordance under a working sign-in.
  const [open, setOpen] = useState(!summary.configured);
  // Editing existing creds jumps straight to the input; first-time setup
  // starts at the developer-site step.
  const [step, setStep] = useState(summary.configured ? TOTAL_STEPS - 1 : 0);
  const [clientId, setClientId] = useState(summary.overrideClientId ?? '');
  const [clientSecret, setClientSecret] = useState('');
  const [saving, setSaving] = useState(false);

  const copiedLabel = t('common.copied', { defaultValue: 'Copied' });
  const platformLabel = t(`chat.platforms.${summary.provider}`, summary.provider);

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

  const stepTitle = [
    t('chat.oauth.wizard.step1Title', { defaultValue: 'Open the developer site' }),
    t('chat.oauth.wizard.step2Title', { defaultValue: 'Set up the app' }),
    t('chat.oauth.wizard.step3Title', { defaultValue: 'Enter your credentials' }),
  ][step];

  const renderConsoleField = (cf: OAuthConsoleField, index: number): React.ReactElement => (
    <div key={`${cf.label}-${index}`} className="rounded-md border border-border-subtle bg-bg-base p-2">
      <div className="flex items-center justify-between gap-2">
        <span className="text-xs text-text-tertiary">{cf.label}</span>
        {cf.copyable && (
          <Button
            variant="ghost"
            size="sm"
            onClick={() => copyValue(cf.value, copiedLabel)}
            aria-label={t('chat.oauth.copyValue', { defaultValue: 'Copy {{label}}', label: cf.label })}
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
        <div className="px-3 pb-3">
          {/* Progress + title */}
          <div className="flex items-center gap-1.5 py-2" aria-hidden>
            {Array.from({ length: TOTAL_STEPS }, (_, i) => (
              <span
                key={i}
                className={cn(
                  'h-1 flex-1 rounded-full',
                  i <= step ? 'bg-purple-violet-500' : 'bg-border-subtle'
                )}
              />
            ))}
          </div>
          <div className="flex items-center justify-between">
            <h4 className="text-sm font-medium text-text-primary">{stepTitle}</h4>
            <span className="text-xs text-text-tertiary">
              {t('chat.oauth.wizard.stepOf', {
                defaultValue: 'Step {{current}} of {{total}}',
                current: step + 1,
                total: TOTAL_STEPS,
              })}
            </span>
          </div>

          <div className="mt-2 flex flex-col gap-3">
            {step === 0 && (
              <>
                <p className="text-xs text-text-secondary">
                  {t('chat.oauth.wizard.step1Hint', {
                    defaultValue:
                      'You’ll register a free app on {{platform}}’s developer site — the one step that has to happen there. Copy this link and open it in your browser:',
                    platform: platformLabel,
                  })}
                </p>
                {renderConsoleField(
                  {
                    label: t('chat.oauth.portalLabel', {
                      defaultValue: 'Developer portal (open in browser)',
                    }),
                    value: summary.registrationUrl,
                    copyable: true,
                    note: null,
                  },
                  -1
                )}
              </>
            )}

            {step === 1 && (
              <>
                {summary.setup.steps.length > 0 && (
                  <>
                    <p className="text-xs text-text-secondary">
                      {t('chat.oauth.wizard.step2Hint', {
                        defaultValue: 'Do these in order — each link opens the exact page you need:',
                      })}
                    </p>
                    <ol className="flex flex-col gap-3">
                      {summary.setup.steps.map((s, i) => {
                        const url = s.url ?? null;
                        return (
                          <li key={i} className="flex gap-2">
                            <span className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-purple-violet-500 text-[0.65rem] font-semibold text-white">
                              {i + 1}
                            </span>
                            <div className="flex-1 min-w-0">
                              {/* Link first: go to the page, THEN read what to
                                  do there. Instructions after a link read
                                  backwards. */}
                              {url && (
                                <div className="mb-1.5 flex items-center justify-between gap-2 rounded-md border border-border-subtle bg-bg-base p-1.5">
                                  <code className="text-xs text-text-primary break-all select-all">
                                    {url}
                                  </code>
                                  <Button
                                    variant="ghost"
                                    size="sm"
                                    onClick={() => copyValue(url, copiedLabel)}
                                    aria-label={t('chat.oauth.wizard.copyLink', {
                                      defaultValue: 'Copy link',
                                    })}
                                  >
                                    <Copy className="w-3.5 h-3.5" />
                                  </Button>
                                </div>
                              )}
                              <p className="text-xs text-text-secondary">{s.text}</p>
                              {/* Values to paste shown right here, at the step
                                  that asks for them — not at the bottom. */}
                              {s.fields.length > 0 && (
                                <div className="mt-1.5 flex flex-col gap-1.5">
                                  {s.fields.map(renderConsoleField)}
                                </div>
                              )}
                            </div>
                          </li>
                        );
                      })}
                    </ol>
                  </>
                )}
                {summary.setup.consoleFields.length > 0 && (
                  <>
                    <p className="text-xs font-medium text-text-secondary">
                      {t('chat.oauth.wizard.valuesLabel', {
                        defaultValue: 'Values to copy into the form:',
                      })}
                    </p>
                    <div className="flex flex-col gap-2">
                      {summary.setup.consoleFields.map(renderConsoleField)}
                    </div>
                  </>
                )}
              </>
            )}

            {step === 2 && (
              <>
                <p className="text-xs text-text-secondary">
                  {t('chat.oauth.wizard.step3Hint', {
                    defaultValue:
                      'Once the app is created, paste its Client ID here. Stored encrypted on this device.',
                  })}
                </p>
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
              </>
            )}
          </div>

          {/* Navigation */}
          <div className="mt-3 flex items-center justify-between">
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setStep((s) => Math.max(0, s - 1))}
              disabled={step === 0 || saving}
            >
              {t('chat.oauth.wizard.back', { defaultValue: 'Back' })}
            </Button>
            {step < TOTAL_STEPS - 1 ? (
              <Button variant="primary" size="sm" onClick={() => setStep((s) => s + 1)}>
                {t('chat.oauth.wizard.next', { defaultValue: 'Next' })}
              </Button>
            ) : (
              <Button variant="primary" size="sm" onClick={handleSave} disabled={!canSave}>
                {saving
                  ? t('chat.oauth.saving', { defaultValue: 'Saving…' })
                  : t('chat.oauth.saveCredentials', { defaultValue: 'Save and enable sign-in' })}
              </Button>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
