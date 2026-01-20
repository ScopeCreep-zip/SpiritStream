import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal, ModalBody, ModalFooter } from '@/components/ui/Modal';
import { Input } from '@/components/ui/Input';
import { Button } from '@/components/ui/Button';
import { ShieldCheck, Eye, EyeOff } from 'lucide-react';
import { login } from '@/lib/backend/env';

interface LoginModalProps {
  open: boolean;
  onSuccess: () => void;
}

/**
 * Login modal for cookie-based authentication.
 * Prompts the user for their API token when authentication is required.
 */
export function LoginModal({ open, onSuccess }: LoginModalProps) {
  const { t } = useTranslation();
  const [token, setToken] = useState('');
  const [showToken, setShowToken] = useState(false);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  // Reset form when modal opens
  useEffect(() => {
    if (open) {
      setToken('');
      setShowToken(false);
      setError('');
      setLoading(false);
    }
  }, [open]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError('');

    if (!token.trim()) {
      setError(t('login.tokenRequired', 'API token is required'));
      return;
    }

    setLoading(true);

    try {
      const success = await login(token.trim());

      if (success) {
        setToken('');
        onSuccess();
      } else {
        setError(t('login.invalidToken', 'Invalid API token'));
      }
    } catch (err) {
      setError(t('login.connectionError', 'Connection error. Please try again.'));
    } finally {
      setLoading(false);
    }
  };

  return (
    <Modal
      open={open}
      onClose={() => {}}
      title={t('login.title', 'Authentication Required')}
    >
      <form onSubmit={handleSubmit}>
        <ModalBody>
          <div className="flex items-center gap-3 mb-4 p-3 bg-[var(--bg-muted)] rounded-lg">
            <ShieldCheck className="w-5 h-5 text-[var(--primary)]" />
            <p className="text-sm text-[var(--text-secondary)]">
              {t(
                'login.description',
                'This SpiritStream server requires authentication. Please enter your API token to continue.'
              )}
            </p>
          </div>

          <div className="space-y-4">
            <div className="relative">
              <Input
                label={t('login.tokenLabel', 'API Token')}
                type={showToken ? 'text' : 'password'}
                value={token}
                onChange={(e) => setToken(e.target.value)}
                placeholder={t('login.tokenPlaceholder', 'Enter your API token')}
                autoFocus
                autoComplete="off"
                disabled={loading}
              />
              <button
                type="button"
                onClick={() => setShowToken(!showToken)}
                aria-label={showToken ? t('common.hidePassword') : t('common.showPassword')}
                aria-pressed={showToken}
                className="absolute right-3 top-[34px] text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors"
                disabled={loading}
              >
                {showToken ? (
                  <EyeOff className="w-4 h-4" aria-hidden="true" />
                ) : (
                  <Eye className="w-4 h-4" aria-hidden="true" />
                )}
              </button>
            </div>

            {error && (
              <div className="p-3 bg-[var(--error-subtle)] border border-[var(--error-border)] rounded-lg">
                <p className="text-sm text-[var(--error-text)]">{error}</p>
              </div>
            )}

            <div className="text-xs text-[var(--text-tertiary)]">
              <p>
                {t(
                  'login.tokenHint',
                  'The API token is configured on the server via the SPIRITSTREAM_API_TOKEN environment variable or in settings.'
                )}
              </p>
            </div>
          </div>
        </ModalBody>

        <ModalFooter>
          <Button type="submit" variant="primary" disabled={loading}>
            {loading
              ? t('login.authenticating', 'Authenticating...')
              : t('login.submit', 'Login')}
          </Button>
        </ModalFooter>
      </form>
    </Modal>
  );
}
