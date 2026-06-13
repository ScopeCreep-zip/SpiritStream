import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Copy } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { events } from '@spiritstream/api-client';
import { toast } from '@/hooks/useToast';
import { logger } from '@/lib/logger';

interface DeviceCodePanelProps {
  /** Backend-provided values from the device-flow start — rendered
   *  verbatim; the panel holds no provider knowledge. */
  userCode: string;
  verificationUri: string;
  expiresIn: number;
  /** Whether the backend already opened the verification page in the
   *  user's browser. The webview can't open external URLs itself
   *  (`shell:open` is denied), so the open happens server-side; this
   *  tells us whether to say "we opened it" or show the copy-link
   *  fallback. */
  browserOpened: boolean;
  /** Called when the backend reports the sign-in finished (success or
   *  failure) so the parent can clear the panel. */
  onFinished: () => void;
}

/**
 * Pure presentation of an RFC 8628 device sign-in: show the short code
 * + verification link, then wait for the backend's `oauth_complete` /
 * `oauth_error` events (the backend owns the polling). On
 * `oauth_complete` the app-level `useOAuthCompletion` hook reloads the
 * active profile, which flips the parent button to "Signed in as …".
 *
 * The verification URL is rendered as selectable text with a copy
 * button — NOT a clickable `target="_blank"` link. In the Tauri webview
 * that anchor is routed to `shell.open`, which capability denies, so it
 * threw "shell.open not allowed". The backend opens the page instead.
 */
export function DeviceCodePanel({
  userCode,
  verificationUri,
  expiresIn,
  browserOpened,
  onFinished,
}: DeviceCodePanelProps): React.ReactElement {
  const { t } = useTranslation();
  const [secondsLeft, setSecondsLeft] = useState(expiresIn);

  useEffect(() => {
    const handle = window.setInterval(() => {
      setSecondsLeft((s) => (s > 0 ? s - 1 : 0));
    }, 1000);
    return () => window.clearInterval(handle);
  }, []);

  useEffect(() => {
    if (secondsLeft === 0) onFinished();
  }, [secondsLeft, onFinished]);

  useEffect(() => {
    let cancelled = false;
    let unlistenComplete: (() => void) | null = null;
    let unlistenError: (() => void) | null = null;

    const setup = async (): Promise<void> => {
      const complete = await events.on('oauth_complete', () => onFinished());
      if (cancelled) {
        complete();
        return;
      }
      unlistenComplete = complete;

      const errored = await events.on<{ reason?: string }>('oauth_error', (payload) => {
        toast.error(
          t('chat.oauth.deviceFailed', {
            defaultValue: 'Sign-in did not complete: {{reason}}',
            reason: payload?.reason ?? 'unknown',
          })
        );
        onFinished();
      });
      if (cancelled) {
        errored();
        return;
      }
      unlistenError = errored;
    };
    setup().catch((error) => logger.error('[DeviceCodePanel] listener setup failed:', error));

    return () => {
      cancelled = true;
      if (unlistenComplete) unlistenComplete();
      if (unlistenError) unlistenError();
    };
  }, [onFinished, t]);

  const copy = async (value: string): Promise<void> => {
    try {
      await navigator.clipboard.writeText(value);
      toast.success(t('common.copied'));
    } catch {
      toast.error(t('common.error'));
    }
  };

  return (
    <div className="mt-3 rounded-lg border border-border-default bg-bg-elevated p-4">
      <p className="text-sm text-text-secondary">
        {browserOpened
          ? t('chat.oauth.deviceOpened', {
              defaultValue:
                'We opened the sign-in page in your browser. Enter this code there:',
            })
          : t('chat.oauth.deviceInstruction', {
              defaultValue: 'Open this page in any browser and enter the code:',
            })}
      </p>
      <div className="mt-2 flex items-center gap-2">
        <span className="text-sm text-text-primary underline break-all select-all">
          {verificationUri}
        </span>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => copy(verificationUri)}
          aria-label={t('chat.oauth.copyLink', { defaultValue: 'Copy link' })}
        >
          <Copy className="w-3.5 h-3.5" />
        </Button>
      </div>
      <div className="mt-3 flex items-center gap-3">
        <code
          aria-live="polite"
          className="text-lg font-semibold tracking-[0.2em] text-text-primary bg-bg-base rounded px-3 py-1.5"
        >
          {userCode}
        </code>
        <Button variant="ghost" size="sm" onClick={() => copy(userCode)}>
          <Copy className="w-3.5 h-3.5" />
          {t('chat.oauth.copyCode', { defaultValue: 'Copy code' })}
        </Button>
      </div>
      <p className="mt-2 text-xs text-text-tertiary">
        {t('chat.oauth.deviceExpires', {
          defaultValue: 'Code expires in {{seconds}}s. This panel closes when sign-in completes.',
          seconds: secondsLeft,
        })}
      </p>
    </div>
  );
}
