import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { Input } from '@/components/ui/Input';
import { Select, SelectOption } from '@/components/ui/Select';
import { Button } from '@/components/ui/Button';
import { useProfileStore } from '@/stores/profileStore';
import { api } from '@/lib/tauri';
import type { Profile, RtmpInput } from '@/types/profile';
import { createDefaultProfile, createDefaultOutputGroup } from '@/types/profile';

export interface ProfileModalProps {
  open: boolean;
  onClose: () => void;
  mode: 'create' | 'edit';
  profile?: Profile;
}

// Resolution options with width/height values
const RESOLUTION_OPTIONS: SelectOption[] = [
  { value: '1920x1080', label: '1080p (1920x1080)' },
  { value: '1280x720', label: '720p (1280x720)' },
  { value: '2560x1440', label: '1440p (2560x1440)' },
  { value: '3840x2160', label: '4K (3840x2160)' },
  { value: '854x480', label: '480p (854x480)' },
];

// Frame rate options
const FPS_OPTIONS: SelectOption[] = [
  { value: '60', label: '60 fps' },
  { value: '30', label: '30 fps' },
  { value: '24', label: '24 fps' },
  { value: '25', label: '25 fps' },
  { value: '50', label: '50 fps' },
];

interface FormData {
  name: string;
  // RTMP Input (structured)
  bindAddress: string;
  port: string;
  application: string;
  // Video settings for default output group
  resolution: string;
  fps: string;
  videoBitrate: string;
}

const defaultFormData: FormData = {
  name: '',
  bindAddress: '0.0.0.0',
  port: '1935',
  application: 'live',
  resolution: '1920x1080',
  fps: '60',
  videoBitrate: '6000',
};

export function ProfileModal({ open, onClose, mode, profile }: ProfileModalProps) {
  const { t } = useTranslation();
  const { updateProfile, saveProfile, current } = useProfileStore();
  const [formData, setFormData] = useState<FormData>(defaultFormData);
  const [errors, setErrors] = useState<Partial<FormData>>({});
  const [saving, setSaving] = useState(false);

  // Initialize form data when modal opens or profile changes
  useEffect(() => {
    if (open) {
      if (mode === 'edit' && profile) {
        const firstGroup = profile.outputGroups[0];
        // Parse resolution from video settings (width x height)
        const resolution = firstGroup?.video
          ? `${firstGroup.video.width}x${firstGroup.video.height}`
          : '1920x1080';
        // Parse bitrate from string (e.g., "6000k" -> "6000")
        const bitrateStr = firstGroup?.video?.bitrate || '6000k';
        const videoBitrate = bitrateStr.replace(/[^\d]/g, '') || '6000';

        setFormData({
          name: profile.name,
          bindAddress: profile.input.bindAddress,
          port: String(profile.input.port),
          application: profile.input.application,
          resolution,
          fps: String(firstGroup?.video?.fps || 60),
          videoBitrate,
        });
      } else {
        setFormData(defaultFormData);
      }
      setErrors({});
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

    const bitrate = parseInt(formData.videoBitrate);
    if (isNaN(bitrate) || bitrate < 500 || bitrate > 50000) {
      newErrors.videoBitrate = t('validation.bitrateRange');
    }

    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  // Validate port conflict with other profiles (Story 2.2)
  const validatePortConflict = async (): Promise<boolean> => {
    const profileId = mode === 'edit' && profile ? profile.id : '';
    const input: RtmpInput = {
      type: 'rtmp',
      bindAddress: formData.bindAddress,
      port: parseInt(formData.port),
      application: formData.application,
    };

    try {
      await api.profile.validateInput(profileId, input);
      return true;
    } catch (error) {
      setErrors((prev) => ({ ...prev, port: String(error) }));
      return false;
    }
  };

  const handleSave = async () => {
    if (!validate()) return;

    setSaving(true);
    try {
      // Validate port conflict before saving (Story 2.2)
      const portOk = await validatePortConflict();
      if (!portOk) {
        setSaving(false);
        return;
      }

      // Parse resolution into width/height
      const [width, height] = formData.resolution.split('x').map(Number);

      // Build RTMP input object
      const input: RtmpInput = {
        type: 'rtmp',
        bindAddress: formData.bindAddress,
        port: parseInt(formData.port),
        application: formData.application,
      };

      if (mode === 'create') {
        // Create new profile with default structure
        const newProfile = createDefaultProfile(formData.name);
        newProfile.input = input;

        // Create default output group with video settings from form
        const outputGroup = createDefaultOutputGroup();
        outputGroup.name = t('outputs.defaultOutputName');
        outputGroup.video = {
          codec: 'libx264',
          width,
          height,
          fps: parseInt(formData.fps),
          bitrate: `${formData.videoBitrate}k`,
          preset: 'veryfast',
          profile: 'high',
        };

        newProfile.outputGroups = [outputGroup];

        // Save to backend via store
        await api.profile.save(newProfile);
        // Reload profiles to update the list
        const { loadProfiles, loadProfile } = useProfileStore.getState();
        await loadProfiles();
        await loadProfile(newProfile.name);
      } else if (mode === 'edit' && current) {
        // Update existing profile's input settings
        const updatedInput = { ...input };

        // Update first output group video settings if it exists
        const updatedOutputGroups = current.outputGroups.map((group, index) => {
          if (index === 0) {
            return {
              ...group,
              video: {
                ...group.video,
                width,
                height,
                fps: parseInt(formData.fps),
                bitrate: `${formData.videoBitrate}k`,
              },
            };
          }
          return group;
        });

        // If no output groups, create one
        if (updatedOutputGroups.length === 0) {
          const outputGroup = createDefaultOutputGroup();
          outputGroup.name = t('outputs.defaultOutputName');
          outputGroup.video = {
            codec: 'libx264',
            width,
            height,
            fps: parseInt(formData.fps),
            bitrate: `${formData.videoBitrate}k`,
            preset: 'veryfast',
            profile: 'high',
          };
          updatedOutputGroups.push(outputGroup);
        }

        updateProfile({
          name: formData.name,
          input: updatedInput,
          outputGroups: updatedOutputGroups,
        });

        // Save to backend
        await saveProfile();
      }

      onClose();
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
    };

  const title = mode === 'create' ? t('modals.createNewProfile') : t('modals.editProfile');

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
        </div>

        {/* Default Output Settings */}
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px' }}>
          <Select
            label={t('encoder.resolution')}
            value={formData.resolution}
            onChange={handleChange('resolution')}
            options={RESOLUTION_OPTIONS}
          />

          <Select
            label={t('encoder.frameRate')}
            value={formData.fps}
            onChange={handleChange('fps')}
            options={FPS_OPTIONS}
          />
        </div>

        <Input
          label={t('encoder.videoBitrate')}
          type="number"
          placeholder="6000"
          value={formData.videoBitrate}
          onChange={handleChange('videoBitrate')}
          error={errors.videoBitrate}
          helper={t('encoder.videoBitrateHelper')}
        />
      </div>
    </Modal>
  );
}
