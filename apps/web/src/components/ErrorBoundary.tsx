import { Component, type ErrorInfo, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

interface Props {
  children: ReactNode;
}

interface State {
  error: Error | null;
  copied: boolean;
}

/**
 * Root-level Error Boundary.
 *
 * Wraps the app shell so a React component crash doesn't leave the
 * user staring at a blank page. The fallback UI uses calm colors
 * (no red alarm), explains what happened, and offers two actions:
 *
 * 1. Copy the error + stack to clipboard (for bug reports).
 * 2. Reload the app.
 *
 * No auto-report-home telemetry. The user explicitly chooses to
 * share the error.
 */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null, copied: false };

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    // Log to the console for local debugging; do NOT auto-ship anywhere.
    // The audit log captures system-state events; component crashes are
    // ephemeral and surface only when the user clicks "Copy details".
    console.error('[ErrorBoundary] Render crash:', error, info);
  }

  reset = () => {
    this.setState({ error: null, copied: false });
  };

  copy = async () => {
    if (!this.state.error) return;
    const payload = `${this.state.error.message}\n\n${this.state.error.stack ?? '(no stack)'}`;
    try {
      await navigator.clipboard.writeText(payload);
      this.setState({ copied: true });
    } catch {
      // Older browsers / restricted contexts — fall through silently.
    }
  };

  render() {
    if (!this.state.error) return this.props.children;
    return <ErrorFallback error={this.state.error} copied={this.state.copied} onCopy={this.copy} onReset={this.reset} />;
  }
}

interface FallbackProps {
  error: Error;
  copied: boolean;
  onCopy: () => void;
  onReset: () => void;
}

function ErrorFallback({ error, copied, onCopy, onReset }: FallbackProps) {
  const { t } = useTranslation();
  return (
    <div
      role="alert"
      aria-live="polite"
      className="min-h-screen flex items-center justify-center bg-bg-base p-6"
    >
      <div className="max-w-lg w-full flex flex-col gap-4 p-6 rounded-xl bg-bg-surface border border-border-default shadow-lg">
        <h1 className="text-xl font-semibold text-text-primary">
          {t('errorBoundary.title', 'Something went wrong')}
        </h1>
        <p className="text-sm text-text-secondary">
          {t(
            'errorBoundary.body',
            'A component on this page hit an unexpected error. Your data on disk is unaffected. Reloading should restore the app.',
          )}
        </p>
        <details className="text-xs text-text-tertiary">
          <summary className="cursor-pointer font-medium">
            {t('errorBoundary.detailsLabel', 'Show technical details')}
          </summary>
          <pre className="mt-2 p-2 rounded bg-bg-sunken overflow-x-auto whitespace-pre-wrap break-words">
            {error.message}
            {error.stack && `\n\n${error.stack}`}
          </pre>
        </details>
        <div className="flex gap-2 mt-2">
          <button
            type="button"
            onClick={onCopy}
            className="px-3 py-2 rounded border border-border-default text-text-primary text-sm hover:bg-bg-hover"
          >
            {copied
              ? t('errorBoundary.copied', 'Copied')
              : t('errorBoundary.copy', 'Copy details')}
          </button>
          <button
            type="button"
            onClick={() => window.location.reload()}
            className="px-3 py-2 rounded bg-primary hover:bg-primary-hover text-primary-foreground text-sm"
          >
            {t('errorBoundary.reload', 'Reload')}
          </button>
          <button
            type="button"
            onClick={onReset}
            className="ms-auto px-3 py-2 rounded text-sm text-text-secondary hover:text-text-primary"
          >
            {t('errorBoundary.dismiss', 'Dismiss')}
          </button>
        </div>
      </div>
    </div>
  );
}
