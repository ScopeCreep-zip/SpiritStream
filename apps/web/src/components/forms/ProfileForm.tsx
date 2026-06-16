import React, { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
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

export interface ProfileFormProps {
  mode: 'create' | 'edit';
  profile?: Profile;
  /** Called after a successful save — closes the host modal / settings window. */
  onDone: () => void;
  /** Cancel affordance. The create modal passes its onClose; the rail passes closeSettings. */
  onCancel?: () => void;
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
 * Profile create / edit FORM body — fields, unified form state + validation,
 * and persistence dispatch. No outer Modal of its own, so it mounts equally
 * as a create-modal body and as the "Edit profile" section of the unified
 * settings window. The RTMP input and password blocks are focused sibling
 * components under `components/forms/`.
 *
 * Resets on mount (and when the target profile changes) — the host mounts this
 * only when visible, so mount === "form opened".
 */
export function ProfileForm({ mode, profile, onDone, onCancel }: ProfileFormProps): React.ReactElement {
  const { t } = useTranslation();
  const { updateProfile, current } = useProfileStore();
  const form = useFormState<FormData>(defaultFormData);
  const formData = form.values;
  const [saving, setSaving] = useState(false);
  const [showPassword, setShowPassword] = useState(false);
  const [serverError, setServerError] = useState<string | undefined>();

  // Validation rules via useFormValidation. UI-state checks only — semantic
  // validation (weak-password policy, runtime bind failures) stays on the backend and
  // surfaces via `CoreError::ValidationFailed`.
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
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode, profile]);

  const persistProfile = async () => {
    // Trim the name on save — the validator only checks `!v.name.trim()`;
    // without the trim here, leading/trailing spaces survive and the
    // persisted name visually disagrees with what the user typed-and-saw.
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
      // updateProfile saves internally; awaiting it lets a backend rejection
      // land in handleSave's catch instead of floating.
      await updateProfile({ name: trimmedName, input });
    }

    onDone();
  };

  const handleSave = async () => {
    if (!validate()) return;
    setServerError(undefined);
    setSaving(true);
    try {
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
      if (field === 'port' && serverError) setServerError(undefined);
    };

  const handleUsePasswordChange = (checked: boolean): void => {
    form.merge({
      usePassword: checked,
      password: checked ? formData.password : '',
      confirmPassword: checked ? formData.confirmPassword : '',
    });
    if (!checked) {
      // Re-running validate after merge would re-display errors for other
      // untouched fields; let the next submit refresh them instead.
      clearErrors();
    }
  };

  const saveLabel = (() => {
    if (saving) return t('common.saving');
    if (mode === 'create') return t('modals.createProfile');
    return t('common.saveChanges');
  })();

  return (
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

      <div className="flex justify-end gap-3 pt-4 mt-2 border-t border-border-muted">
        {onCancel && (
          <Button variant="ghost" onClick={onCancel} disabled={saving}>
            {t('common.cancel')}
          </Button>
        )}
        <Button onClick={() => handleSave()} disabled={saving}>
          {saveLabel}
        </Button>
      </div>
    </div>
  );
}
