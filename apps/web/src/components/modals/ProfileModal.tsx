import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { Input } from '@/components/ui/Input';
import { Button } from '@/components/ui/Button';
import { useProfileStore } from '@/stores/profileStore';
import { api } from '@/lib/client';
import { useFormState, useFormValidation } from '@spiritstream/ui';
import type { ValidationRule } from '@spiritstream/ui';
import type { Profile, RtmpInput } from '@spiritstream/types';
import { createDefaultProfile } from '@/lib/profile-helpers';
import { clientConfig } from '@/lib/constants';
import { RtmpInputForm } from '@/components/forms/RtmpInputForm';
import { ProfilePasswordForm } from '@/components/forms/ProfilePasswordForm';

export interface ProfileModalProps {
  open: boolean;
  onClose: () => void;
  mode: 'create' | 'edit';
  profile?: Profile;
}

interface FormData {
  name: string;
  bindAddress: string;
  port: string;
  application: string;
  usePassword: boolean;
  password: string;
  confirmPassword: string;
}

const defaultFormData: FormData = {
  name: '',
  bindAddress: '0.0.0.0',
  port: '1935',
  application: 'live',
  usePassword: false,
  password: '',
  confirmPassword: '',
};

/**
 * Profile create / edit modal. Owns the unified form state and validation,
 * the port-conflict probe + confirmation modal, and persistence dispatch.
 * The RTMP input block and password block are extracted into focused
 * sibling components under `components/forms/`.
 */
export function ProfileModal({ open, onClose, mode, profile }: ProfileModalProps) {
  const { t } = useTranslation();
  const tDynamic = t as (key: string, options?: { defaultValue?: string }) => string;
  const { updateProfile, saveProfile, current } = useProfileStore();
  const form = useFormState<FormData>(defaultFormData);
  const formData = form.values;
  const [portConflictMessage, setPortConflictMessage] = useState<string | undefined>();
  const [portConflictOpen, setPortConflictOpen] = useState(false);
  const [saving, setSaving] = useState(false);
  const [showPassword, setShowPassword] = useState(false);
  const [serverError, setServerError] = useState<string | undefined>();

  // Validation rules via useFormValidation. UI-state checks only —
  // semantic validation (port conflicts, weak-password policy) stays on
  // the backend and surfaces via `CoreError::ValidationFailed`.
  const rules: Partial<Record<keyof FormData, ValidationRule<FormData>>> = {
    name: (v) => (!v.name.trim() ? t('validation.profileNameRequired') : null),
    bindAddress: (v) => (!v.bindAddress.trim() ? t('validation.bindAddressRequired') : null),
    port: (v) => {
      const p = parseInt(v.port);
      return isNaN(p) || p < 1 || p > 65535 ? t('validation.portRange') : null;
    },
    application: (v) => (!v.application.trim() ? t('validation.applicationRequired') : null),
    password: (v) => {
      if (!v.usePassword) return null;
      if (!v.password) return t('validation.passwordRequired');
      if (v.password.length < clientConfig.PASSWORD_MIN_LENGTH) {
        return t('validation.passwordMinLength', { min: clientConfig.PASSWORD_MIN_LENGTH });
      }
      return null;
    },
    confirmPassword: (v) => {
      if (!v.usePassword) return null;
      return v.password !== v.confirmPassword ? t('validation.passwordsDoNotMatch') : null;
    },
  };

  const { errors, validate, clear: clearErrors } = useFormValidation<FormData>(formData, rules);

  useEffect(() => {
    if (open) {
      if (mode === 'edit' && profile) {
        form.reset({
          name: profile.name,
          bindAddress: profile.input.bindAddress,
          port: String(profile.input.port),
          application: profile.input.application,
          usePassword: false,
          password: '',
          confirmPassword: '',
        });
      } else {
        form.reset(defaultFormData);
      }
      clearErrors();
      setPortConflictMessage(undefined);
      setPortConflictOpen(false);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, mode, profile]);

  // Validate port conflict with other profiles.
  const validatePortConflict = async (): Promise<{
    conflictMessage?: string;
    errorMessage?: string;
  }> => {
    const profileId = mode === 'edit' && profile ? profile.id : '';
    const input: RtmpInput = {
      type: 'rtmp',
      bindAddress: formData.bindAddress,
      port: parseInt(formData.port),
      application: formData.application,
      // Recomputed authoritatively server-side on save (refresh_url).
      url: '',
    };

    try {
      await api.profile.validateInput(profileId, input);
      return {};
    } catch (error) {
      const message = String(error);
      if (message.includes('already configured') || message.includes('already in use')) {
        return { conflictMessage: message };
      }
      return { errorMessage: message };
    }
  };

  const persistProfile = async () => {
    // Trim the name on save — the validator already rejects all-whitespace
    // input via `!v.name.trim()` (line ~65), but only checks; without the
    // trim here, leading/trailing spaces survive and the persisted name
    // visually disagrees with what the user typed-and-saw.
    const trimmedName = formData.name.trim();
    const input: RtmpInput = {
      type: 'rtmp',
      bindAddress: formData.bindAddress,
      port: parseInt(formData.port),
      application: formData.application,
      // Recomputed authoritatively server-side on save (refresh_url).
      url: '',
    };

    if (mode === 'create') {
      const newProfile = createDefaultProfile(trimmedName);
      newProfile.input = input;
      const password = formData.usePassword ? formData.password : undefined;
      await api.profile.save(newProfile, password);
      const { loadProfiles, loadProfile } = useProfileStore.getState();
      await loadProfiles();
      await loadProfile(newProfile.name, password);
    } else if (mode === 'edit' && current) {
      updateProfile({ name: trimmedName, input });
      await saveProfile();
    }

    onClose();
  };

  const handleSave = async (skipPortCheck: boolean = false) => {
    if (!validate()) return;
    setServerError(undefined);
    setSaving(true);
    try {
      if (!skipPortCheck) {
        const { conflictMessage, errorMessage } = await validatePortConflict();
        if (errorMessage) {
          setServerError(errorMessage);
          return;
        }
        if (conflictMessage) {
          setPortConflictMessage(conflictMessage);
          setPortConflictOpen(true);
          return;
        }
      }
      await persistProfile();
    } catch (error) {
      setServerError(String(error));
    } finally {
      setSaving(false);
    }
  };

  const handleChange =
    (field: keyof FormData) => (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>) => {
      form.set(field, e.target.value as FormData[typeof field]);
      if ((field === 'bindAddress' || field === 'port') && portConflictMessage) {
        setPortConflictMessage(undefined);
        setPortConflictOpen(false);
      }
      if (field === 'port' && serverError) setServerError(undefined);
    };

  const handleUsePasswordChange = (checked: boolean): void => {
    form.merge({
      usePassword: checked,
      password: checked ? formData.password : '',
      confirmPassword: checked ? formData.confirmPassword : '',
    });
    if (!checked) {
      // Re-running validate after merge would also re-display errors for
      // other untouched fields; let the next submit refresh them instead.
      clearErrors();
    }
  };

  const title = mode === 'create' ? t('modals.createNewProfile') : t('modals.editProfile');

  return (
    <>
      <Modal
        open={open}
        onClose={onClose}
        title={title}
        footer={
          <>
            <Button variant="ghost" onClick={onClose} disabled={saving}>
              {t('common.cancel')}
            </Button>
            <Button onClick={() => handleSave()} disabled={saving}>
              {(() => {
                if (saving) return t('common.saving');
                if (mode === 'create') return t('modals.createProfile');
                return t('common.saveChanges');
              })()}
            </Button>
          </>
        }
      >
        <div className="flex flex-col gap-4">
          {serverError && (
            <div className="p-3 rounded-lg bg-error-subtle border border-error-border text-error-text text-sm">
              {serverError}
            </div>
          )}

          <Input
            label={t('modals.profileName')}
            placeholder={t('modals.profileNamePlaceholder')}
            value={formData.name}
            onChange={handleChange('name')}
            error={errors.name}
          />

          <RtmpInputForm
            bindAddress={formData.bindAddress}
            port={formData.port}
            application={formData.application}
            onBindAddressChange={handleChange('bindAddress')}
            onPortChange={handleChange('port')}
            onApplicationChange={handleChange('application')}
            errors={{
              bindAddress: errors.bindAddress,
              port: errors.port,
              application: errors.application,
            }}
          />

          {mode === 'create' && (
            <ProfilePasswordForm
              usePassword={formData.usePassword}
              password={formData.password}
              confirmPassword={formData.confirmPassword}
              onUsePasswordChange={handleUsePasswordChange}
              onPasswordChange={handleChange('password')}
              onConfirmPasswordChange={handleChange('confirmPassword')}
              showPassword={showPassword}
              setShowPassword={setShowPassword}
              errors={{
                password: errors.password,
                confirmPassword: errors.confirmPassword,
              }}
            />
          )}
        </div>
      </Modal>

      <Modal
        open={portConflictOpen}
        onClose={() => {
          setPortConflictOpen(false);
          setPortConflictMessage(undefined);
        }}
        title={tDynamic('modals.portConflictTitle', { defaultValue: 'Port already in use' })}
        footer={
          <>
            <Button
              variant="ghost"
              onClick={() => {
                setPortConflictOpen(false);
                setPortConflictMessage(undefined);
              }}
            >
              {t('common.cancel')}
            </Button>
            <Button
              onClick={async () => {
                setPortConflictOpen(false);
                setPortConflictMessage(undefined);
                await handleSave(true);
              }}
            >
              {t('common.confirm')}
            </Button>
          </>
        }
      >
        <div className="space-y-3">
          <p className="text-text-secondary">
            {tDynamic('modals.portConflictBody', {
              defaultValue:
                'Another profile is already configured to use this port. Only one profile can listen on a port at a time.',
            })}
          </p>
          {portConflictMessage && (
            <div className="p-3 rounded-lg bg-warning-subtle border border-warning-border text-warning-text text-sm">
              {portConflictMessage}
            </div>
          )}
          <p className="text-text-secondary">
            {tDynamic('modals.portConflictConfirm', {
              defaultValue: 'Do you want to save anyway?',
            })}
          </p>
        </div>
      </Modal>
    </>
  );
}
