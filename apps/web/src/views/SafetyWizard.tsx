import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Shield, AlertTriangle, EyeOff, Users } from 'lucide-react';

/**
 * Four-step safety setup, rendered INLINE as the "Safety wizard" section of the
 * unified settings window (no overlay of its own — the window provides the
 * surface, focus-trap, and Escape). Pure presentation: collects user choices
 * and hands them to the host via `onComplete`. Wiring to profile state (saving
 * the blocklist, toggling anonymous mode, registering the panic hotkey with
 * Tauri) is the host's responsibility — this component owns only the wizard
 * chrome. Finishing saves and stays in the window; it does NOT close it.
 *
 * - Safe defaults if the user changes nothing: panic hotkey randomised
 *   server-side, anonymous mode ON, blocklist empty.
 * - Shoulder-surfing warning at the top of every step.
 */
export interface SafetyWizardResult {
  /** Phrases the user added to their PII blocklist. */
  piiBlocklist: string[];
  /** Whether anonymous logging stays ON (default true). */
  anonymousLogging: boolean;
  /** Whether the user opted into follower-only chat. */
  followerOnlyDefault: boolean;
}

export interface SafetyWizardProps {
  onComplete: (result: SafetyWizardResult) => void;
}

const STEP_KEYS = ['panic', 'blocklist', 'anonymous', 'followerOnly'] as const;
type StepKey = (typeof STEP_KEYS)[number];

export function SafetyWizard({ onComplete }: SafetyWizardProps): React.ReactElement {
  const { t } = useTranslation();
  const [step, setStep] = useState<StepKey>(STEP_KEYS[0]);
  const [blocklistInput, setBlocklistInput] = useState<string[]>(['', '', '', '']);
  const [anonymousLogging, setAnonymousLogging] = useState(true);
  const [followerOnly, setFollowerOnly] = useState(false);

  const stepIndex = STEP_KEYS.indexOf(step);

  const next = () => {
    if (stepIndex < STEP_KEYS.length - 1) {
      setStep(STEP_KEYS[stepIndex + 1]);
    } else {
      // Raw input on purpose: trimming / deduping / dropping empties is
      // core's job (profile save path normalizes), not the frontend's.
      onComplete({
        piiBlocklist: blocklistInput,
        anonymousLogging,
        followerOnlyDefault: followerOnly,
      });
    }
  };

  const back = () => {
    if (stepIndex > 0) setStep(STEP_KEYS[stepIndex - 1]);
  };

  return (
    <div className="flex flex-col gap-4 max-w-lg">
      <header className="flex items-center gap-2">
        <Shield className="w-5 h-5 text-primary" aria-hidden="true" />
        <h2 className="text-lg font-semibold text-text-primary">
          {t('safety.wizard.title', 'Safety setup')}
        </h2>
      </header>

      {/* Shoulder-surfing warning rendered on every step. */}
      <div className="flex items-start gap-2 text-xs text-warning-text bg-warning-subtle border border-warning-border rounded p-2">
        <AlertTriangle className="w-4 h-4 flex-shrink-0 mt-0.5" aria-hidden="true" />
        <p>
          {t(
            'safety.wizard.shoulderSurf',
            'Anyone watching your screen during setup learns what you type here. Complete in private if possible.'
          )}
        </p>
      </div>

      <ol className="flex gap-1 text-xs">
        {STEP_KEYS.map((s, i) => (
          <li
            key={s}
            className={
              i <= stepIndex ? 'flex-1 h-1 rounded bg-primary' : 'flex-1 h-1 rounded bg-border-default'
            }
          />
        ))}
      </ol>

      <main className="min-h-[140px]">
        {step === 'panic' && (
          <section>
            <h3 className="font-semibold text-text-primary mb-2 flex items-center gap-2">
              <Shield className="w-4 h-4" aria-hidden="true" />
              {t('safety.wizard.panicTitle', 'Panic button')}
            </h3>
            <p className="text-sm text-text-secondary mb-3">
              {t(
                'safety.wizard.panicBody',
                'A panic disconnect stops every stream, drops chat connections, and wipes secrets in memory. The keyboard hotkey is configured in Settings → Safety after this wizard.'
              )}
            </p>
            <p className="text-sm text-text-tertiary">
              {t(
                'safety.wizard.panicDefault',
                'Default hotkey: a randomised chord is generated server-side. You can re-pick it any time.'
              )}
            </p>
          </section>
        )}

        {step === 'blocklist' && (
          <section>
            <h3 className="font-semibold text-text-primary mb-2 flex items-center gap-2">
              <AlertTriangle className="w-4 h-4" aria-hidden="true" />
              {t('safety.wizard.blocklistTitle', 'PII blocklist')}
            </h3>
            <p className="text-sm text-text-secondary mb-3">
              {t(
                'safety.wizard.blocklistBody',
                'Add phrases to drop before they reach chat. Real name, deadname, hometown, workplace. Entries are stored encrypted with the rest of your profile.'
              )}
            </p>
            <div className="flex flex-col gap-2">
              {['realName', 'otherNames', 'hometown', 'workplace'].map((field, i) => {
                const label = t(`safety.wizard.blocklistField.${field}`, field);
                return (
                  <input
                    key={field}
                    type="text"
                    placeholder={label}
                    aria-label={label}
                    value={blocklistInput[i] ?? ''}
                    onChange={(e) => {
                      const updated = [...blocklistInput];
                      updated[i] = e.target.value;
                      setBlocklistInput(updated);
                    }}
                    className="w-full px-3 py-2 text-sm rounded border border-border-default bg-bg-sunken focus:outline-none focus:ring-2 focus:ring-ring-default"
                    autoComplete="off"
                  />
                );
              })}
            </div>
          </section>
        )}

        {step === 'anonymous' && (
          <section>
            <h3 className="font-semibold text-text-primary mb-2 flex items-center gap-2">
              <EyeOff className="w-4 h-4" aria-hidden="true" />
              {t('safety.wizard.anonymousTitle', 'Anonymous mode')}
            </h3>
            <p className="text-sm text-text-secondary mb-3">
              {t(
                'safety.wizard.anonymousBody',
                'Chat usernames render as `hash:abcd1234` in logs and exports. You can still decode them locally with your profile salt. A leaked log can NOT.'
              )}
            </p>
            <label className="flex items-center gap-2 text-sm text-text-primary">
              <input
                type="checkbox"
                checked={anonymousLogging}
                onChange={(e) => setAnonymousLogging(e.target.checked)}
                className="w-4 h-4"
              />
              {t('safety.wizard.anonymousToggle', 'Keep anonymous chat logging ON (recommended)')}
            </label>
            {!anonymousLogging && (
              <p className="mt-2 text-xs text-warning-text bg-warning-subtle border border-warning-border rounded p-2">
                {t(
                  'safety.wizard.anonymousWarning',
                  'Disabling anonymous mode means a leaked log file enumerates everyone who appeared in your chat.'
                )}
              </p>
            )}
          </section>
        )}

        {step === 'followerOnly' && (
          <section>
            <h3 className="font-semibold text-text-primary mb-2 flex items-center gap-2">
              <Users className="w-4 h-4" aria-hidden="true" />
              {t('safety.wizard.followerOnlyTitle', 'Follower-only chat default')}
            </h3>
            <p className="text-sm text-text-secondary mb-3">
              {t(
                'safety.wizard.followerOnlyBody',
                'Turn on follower-only chat when connecting. Applied on Twitch (requires reconnecting Twitch once to grant the permission); other platforms will show a notice that they don’t support it. Reduces drive-by harassment.'
              )}
            </p>
            <label className="flex items-center gap-2 text-sm text-text-primary">
              <input
                type="checkbox"
                checked={followerOnly}
                onChange={(e) => setFollowerOnly(e.target.checked)}
                className="w-4 h-4"
              />
              {t('safety.wizard.followerOnlyToggle', 'Enable follower-only mode by default')}
            </label>
          </section>
        )}
      </main>

      <footer className="flex justify-between items-center mt-2">
        <button
          type="button"
          onClick={back}
          disabled={stepIndex === 0}
          className="px-3 py-1.5 text-sm text-text-secondary hover:text-text-primary disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {t('safety.wizard.back', '← Back')}
        </button>
        <span className="text-xs text-text-tertiary">
          {t('safety.wizard.stepCount', '{{current}} of {{total}}', {
            current: stepIndex + 1,
            total: STEP_KEYS.length,
          })}
        </span>
        <button
          type="button"
          onClick={next}
          className="px-4 py-2 rounded-lg bg-primary hover:bg-primary-hover text-primary-foreground text-sm font-medium"
        >
          {stepIndex === STEP_KEYS.length - 1
            ? t('safety.wizard.finish', 'Finish')
            : t('safety.wizard.next', 'Next →')}
        </button>
      </footer>
    </div>
  );
}
