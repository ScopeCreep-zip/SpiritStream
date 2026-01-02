import { useState, useEffect } from 'react';
import { Modal, ModalBody, ModalFooter } from '@/components/ui/Modal';
import { Input } from '@/components/ui/Input';
import { Button } from '@/components/ui/Button';
import { Lock, Eye, EyeOff } from 'lucide-react';

interface PasswordModalProps {
  open: boolean;
  onClose: () => void;
  onSubmit: (password: string) => void;
  mode: 'encrypt' | 'decrypt';
  profileName?: string;
  error?: string;
}

export function PasswordModal({
  open,
  onClose,
  onSubmit,
  mode,
  profileName,
  error,
}: PasswordModalProps) {
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [localError, setLocalError] = useState('');

  // Reset form when modal opens/closes
  useEffect(() => {
    if (open) {
      setPassword('');
      setConfirmPassword('');
      setShowPassword(false);
      setLocalError('');
    }
  }, [open]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setLocalError('');

    if (!password) {
      setLocalError('Password is required');
      return;
    }

    if (mode === 'encrypt') {
      if (password.length < 8) {
        setLocalError('Password must be at least 8 characters');
        return;
      }
      if (password !== confirmPassword) {
        setLocalError('Passwords do not match');
        return;
      }
    }

    onSubmit(password);
  };

  const displayError = error || localError;

  const title = mode === 'encrypt'
    ? 'Encrypt Profile'
    : 'Enter Password';

  const description = mode === 'encrypt'
    ? `Set a password to encrypt "${profileName || 'this profile'}". You will need this password to access the profile.`
    : `"${profileName || 'This profile'}" is encrypted. Enter the password to unlock it.`;

  return (
    <Modal open={open} onClose={onClose} title={title}>
      <form onSubmit={handleSubmit}>
        <ModalBody>
          <div className="flex items-center gap-3 mb-4 p-3 bg-[var(--bg-muted)] rounded-lg">
            <Lock className="w-5 h-5 text-[var(--primary)]" />
            <p className="text-sm text-[var(--text-secondary)]">
              {description}
            </p>
          </div>

          <div className="space-y-4">
            <div className="relative">
              <Input
                label="Password"
                type={showPassword ? 'text' : 'password'}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder={mode === 'encrypt' ? 'Enter a strong password' : 'Enter password'}
                autoFocus
              />
              <button
                type="button"
                onClick={() => setShowPassword(!showPassword)}
                className="absolute right-3 top-[34px] text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors"
              >
                {showPassword ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
              </button>
            </div>

            {mode === 'encrypt' && (
              <Input
                label="Confirm Password"
                type={showPassword ? 'text' : 'password'}
                value={confirmPassword}
                onChange={(e) => setConfirmPassword(e.target.value)}
                placeholder="Confirm your password"
              />
            )}

            {displayError && (
              <div className="p-3 bg-[var(--error-subtle)] border border-[var(--error-border)] rounded-lg">
                <p className="text-sm text-[var(--error-text)]">{displayError}</p>
              </div>
            )}

            {mode === 'encrypt' && (
              <div className="text-xs text-[var(--text-tertiary)]">
                <p className="font-medium mb-1">Password requirements:</p>
                <ul className="list-disc list-inside space-y-0.5">
                  <li>At least 8 characters</li>
                  <li>Keep it safe - there is no recovery option</li>
                </ul>
              </div>
            )}
          </div>
        </ModalBody>

        <ModalFooter>
          <Button type="button" variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" variant="primary">
            {mode === 'encrypt' ? 'Encrypt Profile' : 'Unlock'}
          </Button>
        </ModalFooter>
      </form>
    </Modal>
  );
}
