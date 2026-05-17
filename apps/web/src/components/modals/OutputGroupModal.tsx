import { useState, useEffect, useMemo, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { Input } from '@/components/ui/Input';
import { Select, SelectOption } from '@/components/ui/Select';
import { Button } from '@/components/ui/Button';
import { Toggle } from '@/components/ui/Toggle';
import { useProfileStore } from '@/stores/profileStore';
import { api } from '@/lib/client';
import { logger } from '@/lib/logger';
import { clientConfig } from '@/lib/constants';
import type { OutputGroup, VideoSettings, AudioSettings, ContainerSettings } from '@spiritstream/types';
import type { Encoders } from '@/types/stream';
import { useFormState, useFormValidation } from '@spiritstream/ui';
import type { ValidationRule } from '@spiritstream/ui';

export interface OutputGroupModalProps {
  open: boolean;
  onClose: () => void;
  mode: 'create' | 'edit';
  group?: OutputGroup;
}

// Encoder option values are server-tuned and hydrated at app start from
// `GET /api/v1/system/encoders/presets`. See `lib/encoderPresets.ts`.
import { encoderPresets, getPresetValues, getDefaultPreset } from '@/lib/encoderPresets';

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

const ENCODER_DEFAULT_LABELS: Record<string, string> = {
  h264_vaapi: 'VAAPI (Linux)',
  hevc_vaapi: 'VAAPI HEVC (Linux)',
  av1_vaapi: 'VAAPI AV1 (Linux)',
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
  const form = useFormState<FormData>(defaultFormData);
  const formData = form.values;
  const [saving, setSaving] = useState(false);
  const [serverError, setServerError] = useState<string | undefined>();
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
            form.merge({
              videoCodec: enc.video[0],
              audioCodec: enc.audio[0] || 'aac',
            });
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

        form.reset({
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
        form.reset(defaultFormData);
      }
      clearErrors();
      setServerError(undefined);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
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
      encoderPresets.RESOLUTION_VALUES.map((value) => ({
        value,
        label: tDynamic(`encoder.resolutions.${value}`, { defaultValue: value }),
      })),
    [tDynamic]
  );

  const fpsOptions: SelectOption[] = useMemo(
    () =>
      encoderPresets.FPS_VALUES.map((value) => ({
        value,
        label: tDynamic(`encoder.frameRates.${value}`, { defaultValue: `${value} fps` }),
      })),
    [tDynamic]
  );

  const audioBitrateOptions: SelectOption[] = useMemo(
    () =>
      encoderPresets.AUDIO_BITRATE_VALUES.map((value) => ({
        value,
        label: tDynamic(`audio.bitrates.${value}`, { defaultValue: value }),
      })),
    [tDynamic]
  );

  const audioChannelsOptions: SelectOption[] = useMemo(
    () =>
      encoderPresets.AUDIO_CHANNELS_VALUES.map((value) => {
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
      encoderPresets.AUDIO_SAMPLE_RATE_VALUES.map((value) => {
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
      encoderPresets.CONTAINER_FORMAT_VALUES.map((value) => ({
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
      encoderPresets.H264_PROFILE_VALUES.map((value) => ({
        value,
        label: value.charAt(0).toUpperCase() + value.slice(1),
      })),
    []
  );

  const rules: Partial<Record<keyof FormData, ValidationRule<FormData>>> = {
    name: (v) => (!v.name.trim() ? t('validation.outputGroupNameRequired') : null),
    // Bounds come from `GET /api/v1/system/client-config` —
    // `StreamService::validate_config` enforces the same range on save.
    // The check here only short-circuits the network roundtrip with an
    // inline error message; the backend always re-validates.
    videoBitrate: (v) => {
      const bitrate = parseInt(v.videoBitrate);
      if (
        isNaN(bitrate)
        || bitrate < clientConfig.BITRATE_MIN
        || bitrate > clientConfig.BITRATE_MAX
      ) {
        return t('validation.bitrateRange');
      }
      return null;
    },
    keyframeIntervalSeconds: (v) => {
      if (!v.keyframeIntervalSeconds.trim()) return null;
      const interval = Number(v.keyframeIntervalSeconds);
      if (
        !Number.isFinite(interval)
        || !Number.isInteger(interval)
        || interval < clientConfig.KEYFRAME_MIN
        || interval > clientConfig.KEYFRAME_MAX
      ) {
        return tDynamic('errors.invalidInput', { defaultValue: 'Invalid input' });
      }
      return null;
    },
  };
  const { errors, validate, clear: clearErrors } = useFormValidation<FormData>(formData, rules);

  useEffect(() => {
    if (!presetSupported) {
      if (formData.preset) form.set('preset', '');
      return;
    }

    if (!presetValues.includes(formData.preset)) {
      const nextPreset = getDefaultPreset(formData.videoCodec, presetValues);
      form.set('preset', nextPreset);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [presetSupported, presetValues, formData.preset, formData.videoCodec]);

  const handleSave = async () => {
    if (!validate()) return;
    setServerError(undefined);

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
        preset: presetSupported && formData.preset ? formData.preset : null,
        profile: formData.profile || null,
        keyframeIntervalSeconds: formData.keyframeIntervalSeconds.trim()
          ? Number(formData.keyframeIntervalSeconds)
          : null,
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
        isDefault: mode === 'edit' && group ? group.isDefault : false,
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
      setServerError(String(error));
    } finally {
      setSaving(false);
    }
  };

  const handleChange = useCallback(
    (field: keyof FormData) => (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>) => {
      form.set(field, e.target.value as FormData[typeof field]);
    },
    [form]
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
      maxWidth="600px"
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

        {serverError && (
          <div className="p-3 rounded-lg bg-error-subtle border border-error-border text-error-text text-sm">
            {serverError}
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
            onChange={(checked) => form.set('generatePts', checked)}
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
