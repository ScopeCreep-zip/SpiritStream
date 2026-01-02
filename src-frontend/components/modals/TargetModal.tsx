import { useState, useEffect } from 'react';
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

// Platform options for select
const PLATFORM_OPTIONS: SelectOption[] = [
  { value: 'youtube', label: 'YouTube' },
  { value: 'twitch', label: 'Twitch' },
  { value: 'kick', label: 'Kick' },
  { value: 'facebook', label: 'Facebook Live' },
  { value: 'custom', label: 'Custom RTMP' },
];

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
      newErrors.name = 'Target name is required';
    }

    if (!formData.url.trim()) {
      newErrors.url = 'Server URL is required';
    } else if (!formData.url.startsWith('rtmp://') && !formData.url.startsWith('rtmps://')) {
      newErrors.url = 'URL must start with rtmp:// or rtmps://';
    }

    if (!formData.streamKey.trim()) {
      newErrors.streamKey = 'Stream key is required';
    }

    const port = parseInt(formData.port);
    if (isNaN(port) || port < 1 || port > 65535) {
      newErrors.port = 'Port must be between 1 and 65535';
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

  const title = mode === 'create' ? 'Add Stream Target' : 'Edit Stream Target';

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={title}
      footer={
        <>
          <Button variant="ghost" onClick={onClose} disabled={saving}>
            Cancel
          </Button>
          <Button onClick={handleSave} disabled={saving}>
            {saving ? 'Saving...' : mode === 'create' ? 'Add Target' : 'Save Changes'}
          </Button>
        </>
      }
    >
      <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
        <Select
          label="Platform"
          value={formData.platform}
          onChange={handlePlatformChange}
          options={PLATFORM_OPTIONS}
        />

        <Input
          label="Target Name"
          placeholder="e.g., YouTube Gaming"
          value={formData.name}
          onChange={handleChange('name')}
          error={errors.name}
        />

        <Input
          label="Server URL"
          placeholder="rtmp://a.rtmp.youtube.com/live2"
          value={formData.url}
          onChange={handleChange('url')}
          error={errors.url}
          helper={formData.platform !== 'custom' ? `Default: ${platformConfig[formData.platform].defaultServer}` : undefined}
        />

        <div style={{ position: 'relative' }}>
          <Input
            label="Stream Key"
            type={showStreamKey ? 'text' : 'password'}
            placeholder="Enter your stream key"
            value={formData.streamKey}
            onChange={handleChange('streamKey')}
            error={errors.streamKey}
            helper="Your stream key is encrypted when saved"
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
            {showStreamKey ? 'Hide' : 'Show'}
          </button>
        </div>

        <Input
          label="Port"
          type="number"
          placeholder="1935"
          value={formData.port}
          onChange={handleChange('port')}
          error={errors.port}
          helper="Default RTMP port is 1935"
        />
      </div>
    </Modal>
  );
}
