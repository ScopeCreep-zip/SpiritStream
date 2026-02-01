import { AlertTriangle, RefreshCw } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Button } from './Button';
import { Logo } from '../layout/Logo';
import { getBackendBaseUrl } from '@/lib/backend/env';

interface ConnectionErrorProps {
  onRetry: () => void;
  isRetrying?: boolean;
}

/**
 * Full-screen overlay shown when the backend server is unreachable.
 * Provides a retry button to manually attempt reconnection.
 */
export function ConnectionError({ onRetry, isRetrying = false }: ConnectionErrorProps) {
  const { t } = useTranslation();
  const serverUrl = getBackendBaseUrl();

  return (
    <div className="fixed inset-0 z-[9999] flex items-center justify-center bg-[var(--bg-base)]">
      <div className="max-w-md w-full mx-4 text-center">
        {/* Logo */}
        <div className="flex justify-center mb-8">
          <Logo size="lg" />
        </div>

        {/* Error Icon */}
        <div className="flex justify-center mb-6">
          <div className="w-16 h-16 rounded-full bg-[var(--error-subtle)] flex items-center justify-center">
            <AlertTriangle className="w-8 h-8 text-[var(--error)]" />
          </div>
        </div>

        {/* Title */}
        <h1 className="text-xl font-semibold text-[var(--text-primary)] mb-2">
          {t('connection.errorTitle', { defaultValue: 'Cannot connect to server' })}
        </h1>

        {/* Description */}
        <p className="text-[var(--text-secondary)] mb-4">
          {t('connection.errorDescription', {
            defaultValue: 'The backend server is not responding. Please ensure it is running and try again.',
          })}
        </p>

        {/* Server URL */}
        <div className="mb-6 p-3 rounded-lg bg-[var(--bg-muted)] border border-[var(--border-default)]">
          <p className="text-xs text-[var(--text-tertiary)] mb-1">
            {t('connection.attemptingConnection', { defaultValue: 'Attempting to connect to:' })}
          </p>
          <p className="text-sm font-mono text-[var(--text-primary)] break-all">{serverUrl}</p>
        </div>

        {/* Retry Button */}
        <Button onClick={onRetry} disabled={isRetrying} className="min-w-[120px]">
          <RefreshCw className={`w-4 h-4 ${isRetrying ? 'animate-spin' : ''}`} />
          {isRetrying
            ? t('connection.retrying', { defaultValue: 'Retrying...' })
            : t('connection.retry', { defaultValue: 'Retry' })}
        </Button>

        {/* Help Text */}
        <p className="mt-6 text-xs text-[var(--text-muted)]">
          {t('connection.helpText', {
            defaultValue: 'If the problem persists, check that the server is running and the URL is correct.',
          })}
        </p>
      </div>
    </div>
  );
}
