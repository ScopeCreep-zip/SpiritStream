import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { Input } from '@/components/ui/Input';
import { Select, SelectOption } from '@/components/ui/Select';
import { Button } from '@/components/ui/Button';
import { useProfileStore } from '@/stores/profileStore';
import type { StreamTarget, Platform } from '@/types/profile';
import { PLATFORMS as platformConfig } from '@/types/profile';

export interface TargetModalProps {
  open: boolean;
  onClose: () => void;
  mode: 'create' | 'edit';
  groupId: string;
  target?: StreamTarget;
}

// Platform values (labels added with translation in component)
const PLATFORM_VALUES: Platform[] = ['youtube', 'twitch', 'kick', 'facebook', 'custom'];

interface FormData {
  platform: Platform;
  name: string;
  url: string;
  streamKey: string;
  port: string;
}

const defaultFormData: FormData = {
  platform: 'youtube',
  name: '',
  url: platformConfig.youtube.defaultServer,
  streamKey: '',
  port: '1935',
};

export function TargetModal({ open, onClose, mode, groupId, target }: TargetModalProps) {
  const { t } = useTranslation();
  const { addStreamTarget, updateStreamTarget, saveProfile } = useProfileStore();
  const [formData, setFormData] = useState<FormData>(defaultFormData);
  const [errors, setErrors] = useState<Partial<FormData>>({});
  const [saving, setSaving] = useState(false);
  const [showStreamKey, setShowStreamKey] = useState(false);

  // Initialize form data when modal opens or target changes
  useEffect(() => {
    if (open) {
      if (mode === 'edit' && target) {
        setFormData({
          platform: target.platform,
          name: target.name,
          url: target.url,
          streamKey: target.streamKey,
          port: String(target.port),
        });
      } else {
        setFormData(defaultFormData);
      }
      setErrors({});
      setShowStreamKey(false);
    }
  }, [open, mode, target]);

  // Update URL when platform changes (only in create mode)
  const handlePlatformChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const newPlatform = e.target.value as Platform;
    setFormData((prev) => ({
      ...prev,
      platform: newPlatform,
      // Only update URL if in create mode or URL hasn't been modified
      url: mode === 'create' ? platformConfig[newPlatform].defaultServer : prev.url,
      // Update name suggestion if empty
      name: prev.name || platformConfig[newPlatform].name,
    }));
  };

  const validate = (): boolean => {
    const newErrors: Partial<FormData> = {};

    if (!formData.name.trim()) {
      newErrors.name = t('validation.targetNameRequired');
    }

    if (!formData.url.trim()) {
      newErrors.url = t('validation.serverUrlRequired');
    } else if (!formData.url.startsWith('rtmp://') && !formData.url.startsWith('rtmps://')) {
      newErrors.url = t('validation.urlMustStartWithRtmp');
    }

    if (!formData.streamKey.trim()) {
      newErrors.streamKey = t('validation.streamKeyRequired');
    }

    const port = parseInt(formData.port);
    if (isNaN(port) || port < 1 || port > 65535) {
      newErrors.port = t('validation.portRange');
    }

    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  const handleSave = async () => {
    if (!validate()) return;

    setSaving(true);
    try {
      const targetData: StreamTarget = {
        id: mode === 'edit' && target ? target.id : crypto.randomUUID(),
        platform: formData.platform,
        name: formData.name,
        url: formData.url,
        streamKey: formData.streamKey,
        port: parseInt(formData.port),
      };

      if (mode === 'create') {
        addStreamTarget(groupId, targetData);
      } else if (mode === 'edit' && target) {
        updateStreamTarget(groupId, target.id, targetData);
      }

      // Save to backend
      await saveProfile();
      onClose();
    } catch (error) {
      setErrors({ name: String(error) });
    } finally {
      setSaving(false);
    }
  };

  const handleChange = (field: keyof FormData) => (
    e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>
  ) => {
    setFormData((prev) => ({ ...prev, [field]: e.target.value }));
    // Clear error when user starts typing
    if (errors[field]) {
      setErrors((prev) => ({ ...prev, [field]: undefined }));
    }
  };

  const title = mode === 'create' ? t('modals.addStreamTarget') : t('modals.editStreamTarget');

  // Create translated platform options
  const platformOptions: SelectOption[] = PLATFORM_VALUES.map((value) => ({
    value,
    label: t(`platforms.${value}`),
  }));

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={title}
      footer={
        <>
          <Button variant="ghost" onClick={onClose} disabled={saving}>
            {t('common.cancel')}
          </Button>
          <Button onClick={handleSave} disabled={saving}>
            {saving ? t('common.saving') : mode === 'create' ? t('modals.addTarget') : t('common.saveChanges')}
          </Button>
        </>
      }
    >
      <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
        <Select
          label={t('modals.platform')}
          value={formData.platform}
          onChange={handlePlatformChange}
          options={platformOptions}
        />

        <Input
          label={t('modals.targetName')}
          placeholder={t('modals.targetNamePlaceholder')}
          value={formData.name}
          onChange={handleChange('name')}
          error={errors.name}
        />

        <Input
          label={t('modals.serverUrl')}
          placeholder="rtmp://a.rtmp.youtube.com/live2"
          value={formData.url}
          onChange={handleChange('url')}
          error={errors.url}
          helper={formData.platform !== 'custom' ? `${t('modals.default')}: ${platformConfig[formData.platform].defaultServer}` : undefined}
        />

        <div style={{ position: 'relative' }}>
          <Input
            label={t('targets.streamKey')}
            type={showStreamKey ? 'text' : 'password'}
            placeholder={t('modals.streamKeyPlaceholder')}
            value={formData.streamKey}
            onChange={handleChange('streamKey')}
            error={errors.streamKey}
            helper={t('modals.streamKeyHelper')}
          />
          <button
            type="button"
            onClick={() => setShowStreamKey(!showStreamKey)}
            style={{
              position: 'absolute',
              right: '12px',
              top: '32px',
              background: 'transparent',
              border: 'none',
              cursor: 'pointer',
              color: 'var(--text-tertiary)',
              fontSize: '12px',
              padding: '4px 8px',
            }}
          >
            {showStreamKey ? t('common.hide') : t('common.show')}
          </button>
        </div>

        <Input
          label={t('modals.port')}
          type="number"
          placeholder="1935"
          value={formData.port}
          onChange={handleChange('port')}
          error={errors.port}
          helper={t('modals.portHelper')}
        />
      </div>
    </Modal>
  );
}
