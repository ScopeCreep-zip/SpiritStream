import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { Input } from '@/components/ui/Input';
import { Select, SelectOption } from '@/components/ui/Select';
import { Toggle } from '@/components/ui/Toggle';
import { Button } from '@/components/ui/Button';
import { useProfileStore } from '@/stores/profileStore';
import { api } from '@/lib/tauri';
import type { OutputGroup } from '@/types/profile';
import type { Encoders } from '@/types/stream';

export interface OutputGroupModalProps {
  open: boolean;
  onClose: () => void;
  mode: 'create' | 'edit';
  group?: OutputGroup;
}

// Resolution option values (labels added with translation in component)
const RESOLUTION_VALUES = ['1920x1080', '1280x720', '2560x1440', '3840x2160', '854x480'];

// Frame rate option values
const FPS_VALUES = ['60', '30', '24', '25', '50'];

// Audio bitrate option values
const AUDIO_BITRATE_VALUES = ['320', '256', '192', '128', '96', '64'];

interface FormData {
  name: string;
  videoEncoder: string;
  resolution: string;
  fps: string;
  videoBitrate: string;
  audioCodec: string;
  audioBitrate: string;
  generatePts: boolean;
}

const defaultFormData: FormData = {
  name: '',
  videoEncoder: 'libx264',
  resolution: '1920x1080',
  fps: '60',
  videoBitrate: '6000',
  audioCodec: 'aac',
  audioBitrate: '128',
  generatePts: false,
};

export function OutputGroupModal({ open, onClose, mode, group }: OutputGroupModalProps) {
  const { t } = useTranslation();
  const { addOutputGroup, updateOutputGroup, saveProfile } = useProfileStore();
  const [formData, setFormData] = useState<FormData>(defaultFormData);
  const [errors, setErrors] = useState<Partial<FormData>>({});
  const [saving, setSaving] = useState(false);
  const [encoders, setEncoders] = useState<Encoders>({ video: ['libx264'], audio: ['aac'] });
  const [loadingEncoders, setLoadingEncoders] = useState(false);

  // Load available encoders when modal opens
  useEffect(() => {
    if (open) {
      setLoadingEncoders(true);
      api.system.getEncoders()
        .then((enc) => {
          setEncoders(enc);
          // If no encoder set yet, use first available
          if (mode === 'create' && enc.video.length > 0) {
            setFormData((prev) => ({
              ...prev,
              videoEncoder: enc.video[0],
              audioCodec: enc.audio[0] || 'aac',
            }));
          }
        })
        .catch((err) => {
          console.error('Failed to load encoders:', err);
        })
        .finally(() => {
          setLoadingEncoders(false);
        });
    }
  }, [open, mode]);

  // Initialize form data when modal opens or group changes
  useEffect(() => {
    if (open) {
      if (mode === 'edit' && group) {
        setFormData({
          name: group.name || '',
          videoEncoder: group.videoEncoder,
          resolution: group.resolution,
          fps: String(group.fps),
          videoBitrate: String(group.videoBitrate),
          audioCodec: group.audioCodec,
          audioBitrate: String(group.audioBitrate),
          generatePts: group.generatePts,
        });
      } else {
        setFormData(defaultFormData);
      }
      setErrors({});
    }
  }, [open, mode, group]);

  // Create encoder options from loaded encoders with translations
  // Use type assertion to bypass strict i18n key checking for dynamic keys
  const tDynamic = t as (key: string, options?: { defaultValue: string }) => string;

  const videoEncoderOptions: SelectOption[] = encoders.video.map((enc) => {
    const label = tDynamic(`encoder.encoders.${enc}`, { defaultValue: enc });
    return { value: enc, label };
  });

  const audioEncoderOptions: SelectOption[] = encoders.audio.map((enc) => {
    const label = tDynamic(`audio.codecs.${enc}`, { defaultValue: enc });
    return { value: enc, label };
  });

  // Create translated options arrays
  const resolutionOptions: SelectOption[] = RESOLUTION_VALUES.map((value) => ({
    value,
    label: tDynamic(`encoder.resolutions.${value}`, { defaultValue: value }),
  }));

  const fpsOptions: SelectOption[] = FPS_VALUES.map((value) => ({
    value,
    label: tDynamic(`encoder.frameRates.${value}`, { defaultValue: `${value} fps` }),
  }));

  const audioBitrateOptions: SelectOption[] = AUDIO_BITRATE_VALUES.map((value) => ({
    value,
    label: tDynamic(`audio.bitrates.${value}`, { defaultValue: `${value} kbps` }),
  }));

  const validate = (): boolean => {
    const newErrors: Partial<FormData> = {};

    if (!formData.name.trim()) {
      newErrors.name = t('validation.outputGroupNameRequired');
    }

    const bitrate = parseInt(formData.videoBitrate);
    if (isNaN(bitrate) || bitrate < 500 || bitrate > 50000) {
      newErrors.videoBitrate = t('validation.bitrateRange');
    }

    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  const handleSave = async () => {
    if (!validate()) return;

    setSaving(true);
    try {
      const groupData: OutputGroup = {
        id: mode === 'edit' && group ? group.id : crypto.randomUUID(),
        name: formData.name,
        videoEncoder: formData.videoEncoder,
        resolution: formData.resolution,
        fps: parseInt(formData.fps),
        videoBitrate: parseInt(formData.videoBitrate),
        audioCodec: formData.audioCodec,
        audioBitrate: parseInt(formData.audioBitrate),
        generatePts: formData.generatePts,
        streamTargets: mode === 'edit' && group ? group.streamTargets : [],
      };

      if (mode === 'create') {
        addOutputGroup(groupData);
      } else if (mode === 'edit' && group) {
        updateOutputGroup(group.id, groupData);
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

  const title = mode === 'create' ? t('modals.createOutputGroup') : t('modals.editOutputGroup');

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={title}
      maxWidth="600px"
      footer={
        <>
          <Button variant="ghost" onClick={onClose} disabled={saving}>
            {t('common.cancel')}
          </Button>
          <Button onClick={handleSave} disabled={saving || loadingEncoders}>
            {saving ? t('common.saving') : mode === 'create' ? t('modals.createGroup') : t('common.saveChanges')}
          </Button>
        </>
      }
    >
      <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
        <Input
          label={t('modals.outputGroupName')}
          placeholder={t('modals.outputGroupNamePlaceholder')}
          value={formData.name}
          onChange={handleChange('name')}
          error={errors.name}
        />

        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px' }}>
          <Select
            label={t('encoder.videoEncoder')}
            value={formData.videoEncoder}
            onChange={handleChange('videoEncoder')}
            options={videoEncoderOptions}
            disabled={loadingEncoders}
          />

          <Select
            label={t('modals.audioCodec')}
            value={formData.audioCodec}
            onChange={handleChange('audioCodec')}
            options={audioEncoderOptions}
            disabled={loadingEncoders}
          />
        </div>

        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px' }}>
          <Select
            label={t('encoder.resolution')}
            value={formData.resolution}
            onChange={handleChange('resolution')}
            options={resolutionOptions}
          />

          <Select
            label={t('encoder.frameRate')}
            value={formData.fps}
            onChange={handleChange('fps')}
            options={fpsOptions}
          />
        </div>

        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px' }}>
          <Input
            label={t('encoder.videoBitrate')}
            type="number"
            placeholder="6000"
            value={formData.videoBitrate}
            onChange={handleChange('videoBitrate')}
            error={errors.videoBitrate}
            helper={t('encoder.videoBitrateHelper')}
          />

          <Select
            label={t('modals.audioBitrate')}
            value={formData.audioBitrate}
            onChange={handleChange('audioBitrate')}
            options={audioBitrateOptions}
          />
        </div>

        <div style={{ paddingTop: '8px' }}>
          <Toggle
            label={t('modals.generatePts')}
            description={t('modals.generatePtsDescription')}
            checked={formData.generatePts}
            onChange={(checked) => setFormData((prev) => ({ ...prev, generatePts: checked }))}
          />
        </div>
      </div>
    </Modal>
  );
}
