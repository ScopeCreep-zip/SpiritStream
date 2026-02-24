import { useState, useEffect, useMemo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { Input } from '@/components/ui/Input';
import { Select, SelectOption } from '@/components/ui/Select';
import { Button } from '@/components/ui/Button';
import { Toggle } from '@/components/ui/Toggle';
import { useProfileStore } from '@/stores/profileStore';
import { api } from '@/lib/backend';
import { logger } from '@/lib/logger';
import type { OutputGroup, VideoSettings, AudioSettings, ContainerSettings } from '@/types/profile';
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

// Audio bitrate option values (with 'k' suffix for new structure)
const AUDIO_BITRATE_VALUES = ['320k', '256k', '192k', '160k', '128k', '96k', '64k'];

// Audio channels options
const AUDIO_CHANNELS_VALUES = ['1', '2', '6', '8'];

// Audio sample rate options
const AUDIO_SAMPLE_RATE_VALUES = ['48000', '44100', '32000'];

// Container format options
const CONTAINER_FORMAT_VALUES = ['flv', 'mpegts', 'mp4'];

interface FormData {
  name: string;
  generatePts: boolean;
  // Video settings (nested)
  videoCodec: string;
  resolution: string;
  fps: string;
  videoBitrate: string;
  preset: string;
  profile: string;
  keyframeIntervalSeconds: string;
  // Audio settings (nested)
  audioCodec: string;
  audioBitrate: string;
  audioChannels: string;
  audioSampleRate: string;
  // Container settings (nested)
  containerFormat: string;
}

// Preset option values
const PRESET_VALUES = [
  'ultrafast',
  'superfast',
  'veryfast',
  'faster',
  'fast',
  'medium',
  'slow',
  'slower',
  'veryslow',
];

const NVENC_PRESET_VALUES = ['p1', 'p2', 'p3', 'p4', 'p5', 'p6', 'p7'];

const AMF_PRESET_VALUES = ['quality', 'balanced', 'speed'];

const ENCODER_DEFAULT_LABELS: Record<string, string> = {
  h264_vaapi: 'VAAPI (Linux)',
  hevc_vaapi: 'VAAPI HEVC (Linux)',
  av1_vaapi: 'VAAPI AV1 (Linux)',
};

// Profile option values
const PROFILE_VALUES = ['baseline', 'main', 'high'];

const getPresetValues = (codec: string): string[] => {
  const normalized = codec.toLowerCase();
  if (normalized.includes('nvenc')) {
    return NVENC_PRESET_VALUES;
  }
  if (normalized === 'libx264' || normalized === 'libx265') {
    return PRESET_VALUES;
  }
  if (normalized.includes('amf')) {
    return AMF_PRESET_VALUES;
  }
  return [];
};

const getDefaultPreset = (codec: string, presetValues: string[]): string => {
  const normalized = codec.toLowerCase();
  if (normalized.includes('nvenc')) {
    return 'p4';
  }
  if (normalized.includes('amf')) {
    return 'balanced';
  }
  if (presetValues.includes('veryfast')) {
    return 'veryfast';
  }
  return presetValues[0] || '';
};

const defaultFormData: FormData = {
  name: '',
  generatePts: true,
  videoCodec: 'libx264',
  resolution: '1920x1080',
  fps: '60',
  videoBitrate: '6000',
  preset: 'veryfast',
  profile: 'high',
  keyframeIntervalSeconds: '',
  audioCodec: 'aac',
  audioBitrate: '160k',
  audioChannels: '2',
  audioSampleRate: '48000',
  containerFormat: 'flv',
};

export function OutputGroupModal({ open, onClose, mode, group }: OutputGroupModalProps) {
  const { t } = useTranslation();
  const { addOutputGroup, updateOutputGroup } = useProfileStore();
  const [formData, setFormData] = useState<FormData>(defaultFormData);
  const [errors, setErrors] = useState<Partial<FormData>>({});
  const [saving, setSaving] = useState(false);
  const [encoders, setEncoders] = useState<Encoders>({ video: ['libx264'], audio: ['aac'] });
  const [loadingEncoders, setLoadingEncoders] = useState(false);

  // Check if trying to edit the default (immutable) group
  const isDefaultGroup = mode === 'edit' && group?.isDefault === true;

  // Close modal when attempting to edit the default (immutable) group
  useEffect(() => {
    if (isDefaultGroup && open) {
      onClose();
    }
  }, [isDefaultGroup, open, onClose]);

  // Load available encoders when modal opens
  useEffect(() => {
    if (open) {
      setLoadingEncoders(true);
      api.system
        .getEncoders()
        .then((enc) => {
          setEncoders(enc);
          // If no encoder set yet, use first available
          if (mode === 'create' && enc.video.length > 0) {
            setFormData((prev) => ({
              ...prev,
              videoCodec: enc.video[0],
              audioCodec: enc.audio[0] || 'aac',
            }));
          }
        })
        .catch((err) => {
          logger.error('Failed to load encoders:', err);
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
        // Parse video bitrate from string (e.g., "6000k" -> "6000")
        const videoBitrate = group.video.bitrate.replace(/[^\d]/g, '') || '6000';
        // Build resolution string from width x height
        const resolution = `${group.video.width}x${group.video.height}`;

        setFormData({
          name: group.name || '',
          generatePts: group.generatePts !== false, // Default to true if undefined
          videoCodec: group.video.codec,
          resolution,
          fps: String(group.video.fps),
          videoBitrate,
          preset: group.video.preset || 'veryfast',
          profile: group.video.profile || 'high',
          keyframeIntervalSeconds: group.video.keyframeIntervalSeconds
            ? String(group.video.keyframeIntervalSeconds)
            : '',
          audioCodec: group.audio.codec,
          audioBitrate: group.audio.bitrate,
          audioChannels: String(group.audio.channels),
          audioSampleRate: String(group.audio.sampleRate),
          containerFormat: group.container.format,
        });
      } else {
        setFormData(defaultFormData);
      }
      setErrors({});
    }
  }, [open, mode, group]);

  // Create encoder options from loaded encoders with translations
  // Use type assertion to bypass strict i18n key checking for dynamic keys
  const tDynamic = t as (
    key: string,
    options?: { defaultValue?: string; [key: string]: string | number | undefined }
  ) => string;

  const videoCodecOptions: SelectOption[] = useMemo(
    () =>
      encoders.video.map((enc) => {
        const defaultLabel = ENCODER_DEFAULT_LABELS[enc] || enc;
        const label = tDynamic(`encoder.encoders.${enc}`, { defaultValue: defaultLabel });
        return { value: enc, label };
      }),
    [encoders.video, tDynamic]
  );

  const audioCodecOptions: SelectOption[] = useMemo(
    () =>
      encoders.audio.map((enc) => {
        const label = tDynamic(`audio.codecs.${enc}`, { defaultValue: enc });
        return { value: enc, label };
      }),
    [encoders.audio, tDynamic]
  );

  const presetValues = useMemo(
    () => getPresetValues(formData.videoCodec),
    [formData.videoCodec]
  );
  const presetSupported = presetValues.length > 0;

  // Create translated options arrays (memoized to avoid re-creating on every render)
  const resolutionOptions: SelectOption[] = useMemo(
    () =>
      RESOLUTION_VALUES.map((value) => ({
        value,
        label: tDynamic(`encoder.resolutions.${value}`, { defaultValue: value }),
      })),
    [tDynamic]
  );

  const fpsOptions: SelectOption[] = useMemo(
    () =>
      FPS_VALUES.map((value) => ({
        value,
        label: tDynamic(`encoder.frameRates.${value}`, { defaultValue: `${value} fps` }),
      })),
    [tDynamic]
  );

  const audioBitrateOptions: SelectOption[] = useMemo(
    () =>
      AUDIO_BITRATE_VALUES.map((value) => ({
        value,
        label: tDynamic(`audio.bitrates.${value}`, { defaultValue: value }),
      })),
    [tDynamic]
  );

  const audioChannelsOptions: SelectOption[] = useMemo(
    () =>
      AUDIO_CHANNELS_VALUES.map((value) => {
        if (value === '1') {
          return {
            value,
            label: tDynamic('audio.channels.mono', { defaultValue: 'Mono' }),
          };
        }
        if (value === '2') {
          return {
            value,
            label: tDynamic('audio.channels.stereo', { defaultValue: 'Stereo' }),
          };
        }
        return {
          value,
          label: tDynamic('audio.channels.multiple', {
            defaultValue: '{{count}} channels',
            count: value,
          }),
        };
      }),
    [tDynamic]
  );

  const audioSampleRateOptions: SelectOption[] = useMemo(
    () =>
      AUDIO_SAMPLE_RATE_VALUES.map((value) => {
        const khz = parseInt(value, 10) / 1000;
        return {
          value,
          label: tDynamic('audio.sampleRateKHz', { defaultValue: '{{value}} kHz', value: khz }),
        };
      }),
    [tDynamic]
  );

  const containerFormatOptions: SelectOption[] = useMemo(
    () =>
      CONTAINER_FORMAT_VALUES.map((value) => ({
        value,
        label: value.toUpperCase(),
      })),
    []
  );

  const presetOptions: SelectOption[] = useMemo(
    () =>
      presetSupported
        ? presetValues.map((value) => ({
            value,
            label: tDynamic(`encoder.presets.${value}`, {
              defaultValue: value.charAt(0).toUpperCase() + value.slice(1),
            }),
          }))
        : [],
    [presetSupported, presetValues, tDynamic]
  );

  const profileOptions: SelectOption[] = useMemo(
    () =>
      PROFILE_VALUES.map((value) => ({
        value,
        label: value.charAt(0).toUpperCase() + value.slice(1),
      })),
    []
  );

  const validate = (): boolean => {
    const newErrors: Partial<FormData> = {};

    if (!formData.name.trim()) {
      newErrors.name = t('validation.outputGroupNameRequired');
    }

    const bitrate = parseInt(formData.videoBitrate);
    if (isNaN(bitrate) || bitrate < 500 || bitrate > 50000) {
      newErrors.videoBitrate = t('validation.bitrateRange');
    }

    if (formData.keyframeIntervalSeconds.trim()) {
      const interval = Number(formData.keyframeIntervalSeconds);
      if (!Number.isFinite(interval) || interval <= 0 || !Number.isInteger(interval)) {
        newErrors.keyframeIntervalSeconds = tDynamic('errors.invalidInput', {
          defaultValue: 'Invalid input',
        });
      }
    }

    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  useEffect(() => {
    if (!presetSupported) {
      if (formData.preset) {
        setFormData((prev) => ({ ...prev, preset: '' }));
      }
      return;
    }

    if (!presetValues.includes(formData.preset)) {
      const nextPreset = getDefaultPreset(formData.videoCodec, presetValues);
      setFormData((prev) => ({ ...prev, preset: nextPreset }));
    }
  }, [presetSupported, presetValues, formData.preset, formData.videoCodec]);

  const handleSave = async () => {
    if (!validate()) return;

    setSaving(true);
    try {
      // Parse resolution into width/height
      const [width, height] = formData.resolution.split('x').map(Number);

      // Build nested video settings
      const video: VideoSettings = {
        codec: formData.videoCodec,
        width,
        height,
        fps: parseInt(formData.fps),
        bitrate: `${formData.videoBitrate}k`,
        preset: presetSupported && formData.preset ? formData.preset : undefined,
        profile: formData.profile,
        keyframeIntervalSeconds: formData.keyframeIntervalSeconds.trim()
          ? Number(formData.keyframeIntervalSeconds)
          : undefined,
      };

      // Build nested audio settings
      const audio: AudioSettings = {
        codec: formData.audioCodec,
        bitrate: formData.audioBitrate,
        channels: parseInt(formData.audioChannels),
        sampleRate: parseInt(formData.audioSampleRate),
      };

      // Build nested container settings
      const container: ContainerSettings = {
        format: formData.containerFormat,
      };

      const groupData: OutputGroup = {
        id: mode === 'edit' && group ? group.id : crypto.randomUUID(),
        name: formData.name,
        generatePts: formData.generatePts,
        video,
        audio,
        container,
        streamTargets: mode === 'edit' && group ? group.streamTargets : [],
      };

      if (mode === 'create') {
        await addOutputGroup(groupData);
      } else if (mode === 'edit' && group) {
        await updateOutputGroup(group.id, groupData);
      }
      // Note: saveProfile() is called internally by the store functions
      onClose();
    } catch (error) {
      setErrors({ name: String(error) });
    } finally {
      setSaving(false);
    }
  };

  const handleChange = useCallback(
    (field: keyof FormData) => (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>) => {
      setFormData((prev) => ({ ...prev, [field]: e.target.value }));
      // Clear error when user starts typing
      setErrors((prev) => (prev[field] ? { ...prev, [field]: undefined } : prev));
    },
    []
  );

  const title = mode === 'create' ? t('modals.createOutputGroup') : t('modals.editOutputGroup');

  // Don't render anything for the immutable default group
  if (isDefaultGroup) {
    return null;
  }

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={title}
      maxWidth="lg"
      footer={
        <>
          <Button variant="ghost" onClick={onClose} disabled={saving}>
            {t('common.cancel')}
          </Button>
          <Button onClick={handleSave} disabled={saving || loadingEncoders}>
            {saving
              ? t('common.saving')
              : mode === 'create'
                ? t('modals.createGroup')
                : t('common.saveChanges')}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-4">
        {/* Info message explaining custom output groups */}
        {mode === 'create' && (
          <div className="p-3 bg-primary-muted rounded-lg text-sm text-text-secondary leading-normal">
            {tDynamic('modals.outputGroupExplanation', {
              defaultValue: 'Custom output groups re-encode your incoming stream to different settings. Use these when you need to send different quality streams to different platforms. The default passthrough group relays your stream as-is without re-encoding.'
            })}
          </div>
        )}

        <Input
          label={t('modals.outputGroupName')}
          placeholder={t('modals.outputGroupNamePlaceholder')}
          value={formData.name}
          onChange={handleChange('name')}
          error={errors.name}
        />

        {/* Timestamp & Sync Settings */}
        <div className="p-3 bg-bg-muted rounded-lg">
          <Toggle
            checked={formData.generatePts}
            onChange={(checked) => setFormData((prev) => ({ ...prev, generatePts: checked }))}
            label={t('encoder.generatePts')}
            description={t('encoder.generatePtsDescription')}
          />
        </div>

        {/* Video Settings Section */}
        <div className="p-3 bg-bg-muted rounded-lg">
          <div className="mb-3 text-sm font-medium text-text-primary">
            {t('modals.videoSettings')}
          </div>

          <div className="grid grid-cols-2 gap-3 mb-3">
            <Select
              label={t('encoder.videoEncoder')}
              value={formData.videoCodec}
              onChange={handleChange('videoCodec')}
              options={videoCodecOptions}
              disabled={loadingEncoders}
            />

            <Select
              label={t('encoder.resolution')}
              value={formData.resolution}
              onChange={handleChange('resolution')}
              options={resolutionOptions}
            />
          </div>

          <div className="grid grid-cols-3 gap-3 mb-3">
            <Select
              label={t('encoder.frameRate')}
              value={formData.fps}
              onChange={handleChange('fps')}
              options={fpsOptions}
            />

            <Input
              label={t('encoder.videoBitrate')}
              type="number"
              placeholder={t('modals.videoBitratePlaceholder')}
              value={formData.videoBitrate}
              onChange={handleChange('videoBitrate')}
              error={errors.videoBitrate}
            />

            <Select
              label={t('encoder.profile')}
              value={formData.profile}
              onChange={handleChange('profile')}
              options={profileOptions}
            />
          </div>

          <Select
            label={t('encoder.preset')}
            value={formData.preset}
            onChange={handleChange('preset')}
            options={presetOptions}
            disabled={!presetSupported}
          />
          {!presetSupported && (
            <div className="mt-1.5 text-xs text-text-secondary">
              {tDynamic('encoder.presetUnsupported', {
                defaultValue: 'Presets are not available for this encoder.',
              })}
            </div>
          )}

          <Input
            label={t('encoder.keyframeIntervalSeconds')}
            type="number"
            min="1"
            step="1"
            placeholder={t('modals.keyframeIntervalPlaceholder')}
            value={formData.keyframeIntervalSeconds}
            onChange={handleChange('keyframeIntervalSeconds')}
            helper={t('encoder.keyframeIntervalHelper')}
            error={errors.keyframeIntervalSeconds}
          />
        </div>

        {/* Audio Settings Section */}
        <div className="p-3 bg-bg-muted rounded-lg">
          <div className="mb-3 text-sm font-medium text-text-primary">
            {t('modals.audioSettings')}
          </div>

          <div className="grid grid-cols-2 gap-3 mb-3">
            <Select
              label={t('modals.audioCodec')}
              value={formData.audioCodec}
              onChange={handleChange('audioCodec')}
              options={audioCodecOptions}
              disabled={loadingEncoders}
            />

            <Select
              label={t('modals.audioBitrate')}
              value={formData.audioBitrate}
              onChange={handleChange('audioBitrate')}
              options={audioBitrateOptions}
            />
          </div>

          <div className="grid grid-cols-2 gap-3">
            <Select
              label={t('modals.audioChannels')}
              value={formData.audioChannels}
              onChange={handleChange('audioChannels')}
              options={audioChannelsOptions}
            />

            <Select
              label={t('modals.audioSampleRate')}
              value={formData.audioSampleRate}
              onChange={handleChange('audioSampleRate')}
              options={audioSampleRateOptions}
            />
          </div>
        </div>

        {/* Container Settings Section */}
        <div className="p-3 bg-bg-muted rounded-lg">
          <div className="mb-3 text-sm font-medium text-text-primary">
            {t('modals.containerSettings')}
          </div>

          <Select
            label={t('modals.containerFormat')}
            value={formData.containerFormat}
            onChange={handleChange('containerFormat')}
            options={containerFormatOptions}
          />
        </div>
      </div>
    </Modal>
  );
}
