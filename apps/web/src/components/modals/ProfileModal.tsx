import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Lock, Eye, EyeOff } from 'lucide-react';
import { cn } from '@/lib/cn';
import { Modal } from '@/components/ui/Modal';
import { Input } from '@/components/ui/Input';
import { Button } from '@/components/ui/Button';
import { Toggle } from '@/components/ui/Toggle';
import { useProfileStore } from '@/stores/profileStore';
import { api } from '@/lib/client';
import { PasswordInput, useFormState, useFormValidation } from '@spiritstream/ui';
import type { ValidationRule } from '@spiritstream/ui';
import type { Profile, RtmpInput } from '@spiritstream/types';
import { createDefaultProfile } from '@/lib/profile-helpers';
import { clientConfig } from '@/lib/constants';

export interface ProfileModalProps {
  open: boolean;
  onClose: () => void;
  mode: 'create' | 'edit';
  profile?: Profile;
}

interface FormData {
  name: string;
  // RTMP Input (structured)
  bindAddress: string;
  port: string;
  application: string;
  // Password protection
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

  const validation = useFormValidation<FormData>(formData, rules);
  const { errors, validate, clear: clearErrors } = validation;

  // Initialize form data when modal opens or profile changes
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

  // Validate port conflict with other profiles (Story 2.2)
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
    // Build RTMP input object
    const input: RtmpInput = {
      type: 'rtmp',
      bindAddress: formData.bindAddress,
      port: parseInt(formData.port),
      application: formData.application,
    };

    if (mode === 'create') {
      // Create new profile with default passthrough group
      // The default profile factory already includes the passthrough output group
      const newProfile = createDefaultProfile(formData.name);
      newProfile.input = input;

      // Save to backend via store (with password if enabled)
      const password = formData.usePassword ? formData.password : undefined;
      await api.profile.save(newProfile, password);
      // Reload profiles to update the list
      const { loadProfiles, loadProfile } = useProfileStore.getState();
      await loadProfiles();
      // Load profile (will require password if encrypted)
      await loadProfile(newProfile.name, password);
    } else if (mode === 'edit' && current) {
      // Update existing profile's name and input settings only
      // Do NOT modify output groups - those are configured separately
      updateProfile({
        name: formData.name,
        input,
      });

      // Save to backend
      await saveProfile();
    }

    onClose();
  };

  // Server-side errors (port conflict, save failure) surface via toast/conflict
  // modal rather than the per-field rule map — we use a small bypass state
  // for the "save returned an error string" case so the user still sees it.
  const [serverError, setServerError] = useState<string | undefined>();

  const handleSave = async (skipPortCheck: boolean = false) => {
    if (!validate()) return;
    setServerError(undefined);

    setSaving(true);
    try {
      if (!skipPortCheck) {
        // Validate port conflict before saving (Story 2.2)
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
            {saving
              ? t('common.saving')
              : mode === 'create'
                ? t('modals.createProfile')
                : t('common.saveChanges')}
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

        {/* RTMP Input Configuration */}
        <div className="p-3 bg-bg-muted rounded-lg">
          <div className="mb-3 text-sm font-medium text-text-primary">
            {t('modals.rtmpInputSettings')}
          </div>
          <div className="grid grid-cols-[1fr_100px_1fr] gap-3">
            <Input
              label={t('modals.bindAddress')}
              placeholder={t('modals.bindAddressPlaceholder')}
              value={formData.bindAddress}
              onChange={handleChange('bindAddress')}
              error={errors.bindAddress}
              helper={t('modals.bindAddressHelper')}
            />
            <Input
              label={t('modals.port')}
              type="number"
              placeholder={t('modals.portPlaceholder')}
              value={formData.port}
              onChange={handleChange('port')}
              error={errors.port}
            />
            <Input
              label={t('modals.application')}
              placeholder={t('modals.applicationPlaceholder')}
              value={formData.application}
              onChange={handleChange('application')}
              error={errors.application}
              helper={t('modals.applicationHelper')}
            />
          </div>
          <div className="mt-2 text-xs text-text-tertiary">
            {t('modals.rtmpUrlPreview')}: rtmp://{formData.bindAddress}:{formData.port}/
            {formData.application}
          </div>
          <div className="mt-2 p-2 bg-bg-base rounded text-xs text-text-secondary leading-normal">
            {tDynamic('modals.profileExplanation', {
              defaultValue: 'Configure your streaming software (OBS, etc.) to send to this RTMP URL. Encoding settings are configured in your streaming software, not in the profile. Use output groups to re-encode to different settings for different platforms.'
            })}
          </div>
        </div>

        {/* Password Protection (only for create mode) */}
        {mode === 'create' && (
          <div className="p-3 bg-bg-muted rounded-lg">
            <div className={cn('flex items-center justify-between', formData.usePassword && 'mb-3')}>
              <div className="flex items-center gap-2">
                <Lock className="w-4 h-4 text-primary" />
                <span className="text-sm font-medium text-text-primary">
                  {t('profiles.protectWithPassword')}
                </span>
              </div>
              <Toggle
                checked={formData.usePassword}
                onChange={(checked) => {
                  form.merge({
                    usePassword: checked,
                    password: checked ? formData.password : '',
                    confirmPassword: checked ? formData.confirmPassword : '',
                  });
                  if (!checked) {
                    // Re-running validate after merge would also re-display
                    // errors for other untouched fields; instead let the
                    // next submit refresh them.
                    clearErrors();
                  }
                }}
              />
            </div>

            {formData.usePassword && (
              <div className="flex flex-col gap-3">
                <PasswordInput
                  label={t('modals.password.password')}
                  value={formData.password}
                  onChange={handleChange('password')}
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
                  value={formData.confirmPassword}
                  onChange={handleChange('confirmPassword')}
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
                'Another profile is already configured to use this port. Only one profile can listen on a port at a time.'
            })}
          </p>
          {portConflictMessage && (
            <div className="p-3 rounded-lg bg-warning-subtle border border-warning-border text-warning-text text-sm">
              {portConflictMessage}
            </div>
          )}
          <p className="text-text-secondary">
            {tDynamic('modals.portConflictConfirm', {
              defaultValue: 'Do you want to save anyway?'
            })}
          </p>
        </div>
      </Modal>
    </>
  );
}
