import React from 'react';
import { useTranslation } from 'react-i18next';
import { Lock, Eye, EyeOff } from 'lucide-react';
import { Toggle } from '@/components/ui/Toggle';
import { PasswordInput } from '@spiritstream/ui';
import { cn } from '@/lib/cn';

interface ProfilePasswordFormProps {
  usePassword: boolean;
  password: string;
  confirmPassword: string;

  onUsePasswordChange: (checked: boolean) => void;
  onPasswordChange: (event: React.ChangeEvent<HTMLInputElement>) => void;
  onConfirmPasswordChange: (event: React.ChangeEvent<HTMLInputElement>) => void;

  showPassword: boolean;
  setShowPassword: (visible: boolean) => void;

  errors: {
    password?: string;
    confirmPassword?: string;
  };
}

/**
 * Password protection form for new profile creation. Only rendered when
 * mode === 'create' by the parent ProfileModal. Pure presentation: form
 * state + handlers come from the parent.
 */
export function ProfilePasswordForm({
  usePassword,
  password,
  confirmPassword,
  onUsePasswordChange,
  onPasswordChange,
  onConfirmPasswordChange,
  showPassword,
  setShowPassword,
  errors,
}: ProfilePasswordFormProps): React.ReactElement {
  const { t } = useTranslation();

  return (
    <div className="p-3 bg-bg-muted rounded-lg">
      <div className={cn('flex items-center justify-between', usePassword && 'mb-3')}>
        <div className="flex items-center gap-2">
          <Lock className="w-4 h-4 text-primary" />
          <span className="text-sm font-medium text-text-primary">
            {t('profiles.protectWithPassword')}
          </span>
        </div>
        <Toggle checked={usePassword} onChange={onUsePasswordChange} />
      </div>

      {usePassword && (
        <div className="flex flex-col gap-3">
          <PasswordInput
            label={t('modals.password.password')}
            value={password}
            onChange={onPasswordChange}
            error={errors.password}
            placeholder={t('modals.enterStrongPassword')}
            autoComplete="new-password"
            visible={showPassword}
            onVisibilityChange={setShowPassword}
            renderToggleIcon={(visible) =>
              visible ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />
            }
          />

          <PasswordInput
            label={t('modals.confirmPassword')}
            value={confirmPassword}
            onChange={onConfirmPasswordChange}
            error={errors.confirmPassword}
            placeholder={t('modals.confirmYourPassword')}
            autoComplete="new-password"
            visible={showPassword}
            onVisibilityChange={setShowPassword}
            renderToggleIcon={(visible) =>
              visible ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />
            }
          />

          <div className="text-xs text-text-tertiary">
            <p className="font-medium mb-1">{t('modals.passwordRequirements')}:</p>
            <ul className="m-0 ps-4">
              <li>{t('modals.passwordReq8Chars')}</li>
              <li>{t('modals.passwordReqNoRecovery')}</li>
            </ul>
          </div>
        </div>
      )}
    </div>
  );
}
