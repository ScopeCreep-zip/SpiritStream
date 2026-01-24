import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Lock, Eye, EyeOff } from 'lucide-react';
import { Modal } from '@/components/ui/Modal';
import { Input } from '@/components/ui/Input';
import { Button } from '@/components/ui/Button';
import { Toggle } from '@/components/ui/Toggle';
import { useProfileStore } from '@/stores/profileStore';
import { api } from '@/lib/backend';
import type { Profile, RtmpInput } from '@/types/profile';
import { createDefaultProfile, getRtmpInput } from '@/types/profile';

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
  const [formData, setFormData] = useState<FormData>(defaultFormData);
  const [errors, setErrors] = useState<Partial<FormData>>({});
  const [portConflictMessage, setPortConflictMessage] = useState<string | undefined>();
  const [portConflictOpen, setPortConflictOpen] = useState(false);
  const [saving, setSaving] = useState(false);
  const [showPassword, setShowPassword] = useState(false);

  // Initialize form data when modal opens or profile changes
  useEffect(() => {
    if (open) {
      if (mode === 'edit' && profile) {
        const rtmpInput = getRtmpInput(profile);
        setFormData({
          name: profile.name,
          bindAddress: rtmpInput?.bindAddress ?? '0.0.0.0',
          port: String(rtmpInput?.port ?? 1935),
          application: rtmpInput?.application ?? 'live',
          usePassword: false,
          password: '',
          confirmPassword: '',
        });
      } else {
        setFormData(defaultFormData);
      }
      setErrors({});
      setPortConflictMessage(undefined);
      setPortConflictOpen(false);
    }
  }, [open, mode, profile]);

  const validate = (): boolean => {
    const newErrors: Partial<FormData> = {};

    if (!formData.name.trim()) {
      newErrors.name = t('validation.profileNameRequired');
    }

    // Validate bind address
    if (!formData.bindAddress.trim()) {
      newErrors.bindAddress = t('validation.bindAddressRequired');
    }

    // Validate port
    const port = parseInt(formData.port);
    if (isNaN(port) || port < 1 || port > 65535) {
      newErrors.port = t('validation.portRange');
    }

    // Validate application name
    if (!formData.application.trim()) {
      newErrors.application = t('validation.applicationRequired');
    }

    // Validate password when protection is enabled
    if (formData.usePassword) {
      if (!formData.password) {
        newErrors.password = t('validation.passwordRequired');
      } else if (formData.password.length < 8) {
        newErrors.password = t('validation.passwordMinLength');
      }
      if (formData.password !== formData.confirmPassword) {
        newErrors.confirmPassword = t('validation.passwordsDoNotMatch');
      }
    }

    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

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

  const handleSave = async (skipPortCheck: boolean = false) => {
    if (!validate()) return;

    setSaving(true);
    try {
      if (!skipPortCheck) {
        // Validate port conflict before saving (Story 2.2)
        const { conflictMessage, errorMessage } = await validatePortConflict();
        if (errorMessage) {
          setErrors((prev) => ({ ...prev, port: errorMessage }));
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
      setErrors({ name: String(error) });
    } finally {
      setSaving(false);
    }
  };

  const handleChange =
    (field: keyof FormData) => (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>) => {
      setFormData((prev) => ({ ...prev, [field]: e.target.value }));
      // Clear error when user starts typing
      if (errors[field]) {
        setErrors((prev) => ({ ...prev, [field]: undefined }));
      }
      if ((field === 'bindAddress' || field === 'port') && portConflictMessage) {
        setPortConflictMessage(undefined);
        setPortConflictOpen(false);
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
            {saving
              ? t('common.saving')
              : mode === 'create'
                ? t('modals.createProfile')
                : t('common.saveChanges')}
          </Button>
        </>
      }
    >
      <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
        <Input
          label={t('modals.profileName')}
          placeholder={t('modals.profileNamePlaceholder')}
          value={formData.name}
          onChange={handleChange('name')}
          error={errors.name}
        />

        {/* RTMP Input Configuration */}
        <div style={{ padding: '12px', backgroundColor: 'var(--bg-muted)', borderRadius: '8px' }}>
          <div
            style={{
              marginBottom: '12px',
              fontSize: '0.875rem',
              fontWeight: 500,
              color: 'var(--text-primary)',
            }}
          >
            {t('modals.rtmpInputSettings')}
          </div>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 100px 1fr', gap: '12px' }}>
            <Input
              label={t('modals.bindAddress')}
              placeholder="0.0.0.0"
              value={formData.bindAddress}
              onChange={handleChange('bindAddress')}
              error={errors.bindAddress}
              helper={t('modals.bindAddressHelper')}
            />
            <Input
              label={t('modals.port')}
              type="number"
              placeholder="1935"
              value={formData.port}
              onChange={handleChange('port')}
              error={errors.port}
            />
            <Input
              label={t('modals.application')}
              placeholder="live"
              value={formData.application}
              onChange={handleChange('application')}
              error={errors.application}
              helper={t('modals.applicationHelper')}
            />
          </div>
          <div style={{ marginTop: '8px', fontSize: '0.75rem', color: 'var(--text-tertiary)' }}>
            {t('modals.rtmpUrlPreview')}: rtmp://{formData.bindAddress}:{formData.port}/
            {formData.application}
          </div>
          <div style={{
            marginTop: '8px',
            padding: '8px',
            backgroundColor: 'var(--bg-base)',
            borderRadius: '4px',
            fontSize: '0.75rem',
            color: 'var(--text-secondary)',
            lineHeight: '1.5'
          }}>
            {tDynamic('modals.profileExplanation', {
              defaultValue: 'Configure your streaming software (OBS, etc.) to send to this RTMP URL. Encoding settings are configured in your streaming software, not in the profile. Use output groups to re-encode to different settings for different platforms.'
            })}
          </div>
        </div>

        {/* Password Protection (only for create mode) */}
        {mode === 'create' && (
          <div style={{ padding: '12px', backgroundColor: 'var(--bg-muted)', borderRadius: '8px' }}>
            <div className="flex items-center justify-between" style={{ marginBottom: formData.usePassword ? '12px' : '0' }}>
              <div className="flex items-center gap-2">
                <Lock className="w-4 h-4 text-[var(--primary)]" />
                <span style={{ fontSize: '0.875rem', fontWeight: 500, color: 'var(--text-primary)' }}>
                  {t('profiles.protectWithPassword')}
                </span>
              </div>
              <Toggle
                checked={formData.usePassword}
                onChange={(checked) => {
                  setFormData(prev => ({
                    ...prev,
                    usePassword: checked,
                    password: checked ? prev.password : '',
                    confirmPassword: checked ? prev.confirmPassword : ''
                  }));
                  if (!checked) {
                    setErrors(prev => ({ ...prev, password: undefined, confirmPassword: undefined }));
                  }
                }}
              />
            </div>

            {formData.usePassword && (
              <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
                <div className="relative">
                  <Input
                    label={t('modals.password.password')}
                    type={showPassword ? 'text' : 'password'}
                    value={formData.password}
                    onChange={handleChange('password')}
                    error={errors.password}
                    placeholder={t('modals.enterStrongPassword')}
                    autoComplete="new-password"
                  />
                  <button
                    type="button"
                    onClick={() => setShowPassword(!showPassword)}
                    className="absolute right-3 top-[34px] text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors"
                    style={{ background: 'none', border: 'none', cursor: 'pointer', padding: '4px' }}
                  >
                    {showPassword ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                  </button>
                </div>

                <Input
                  label={t('modals.confirmPassword')}
                  type={showPassword ? 'text' : 'password'}
                  value={formData.confirmPassword}
                  onChange={handleChange('confirmPassword')}
                  error={errors.confirmPassword}
                  placeholder={t('modals.confirmYourPassword')}
                  autoComplete="new-password"
                />

                <div style={{ fontSize: '0.75rem', color: 'var(--text-tertiary)' }}>
                  <p style={{ fontWeight: 500, marginBottom: '4px' }}>{t('modals.passwordRequirements')}:</p>
                  <ul style={{ margin: 0, paddingLeft: '16px' }}>
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
          <p className="text-[var(--text-secondary)]">
            {tDynamic('modals.portConflictBody', {
              defaultValue:
                'Another profile is already configured to use this port. Only one profile can listen on a port at a time.'
            })}
          </p>
          {portConflictMessage && (
            <div className="p-3 rounded-lg bg-[var(--warning-subtle)] border border-[var(--warning-border)] text-[var(--warning-text)] text-sm">
              {portConflictMessage}
            </div>
          )}
          <p className="text-[var(--text-secondary)]">
            {tDynamic('modals.portConflictConfirm', {
              defaultValue: 'Do you want to save anyway?'
            })}
          </p>
        </div>
      </Modal>
    </>
  );
}
