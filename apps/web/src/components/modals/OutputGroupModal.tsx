import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { Input } from '@/components/ui/Input';
import { Button } from '@/components/ui/Button';
import { Toggle } from '@/components/ui/Toggle';
import { useProfileStore } from '@/stores/profileStore';
import { api } from '@/lib/client';
import { logger } from '@/lib/logger';
import { clientConfig } from '@/lib/constants';
import type {
  OutputGroup,
  VideoSettings,
  AudioSettings,
  ContainerSettings,
} from '@spiritstream/types';
import type { Encoders } from '@/types/stream';
import { useFormState, useFormValidation } from '@spiritstream/ui';
import type { ValidationRule } from '@spiritstream/ui';
import {
  VideoSettingsForm,
  isPresetSupportedFor,
  type VideoFormValues,
  type VideoFormErrors,
} from '@/components/encoder/VideoSettingsForm';
import { AudioSettingsForm, type AudioFormValues } from '@/components/encoder/AudioSettingsForm';
import {
  ContainerSettingsForm,
  type ContainerFormValues,
} from '@/components/encoder/ContainerSettingsForm';

export interface OutputGroupModalProps {
  open: boolean;
  onClose: () => void;
  mode: 'create' | 'edit';
  group?: OutputGroup;
}

interface FormData extends VideoFormValues, AudioFormValues, ContainerFormValues {
  name: string;
  generatePts: boolean;
}

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

/**
 * Custom output group editor — orchestrator. Owns form state, validation,
 * encoder fetch, and persistence. Visible sections (Video / Audio /
 * Container) are self-contained modules under `components/encoder/`,
 * each owning its own option-building memos and Select wiring.
 *
 * Backend authority: every value here is re-validated by
 * `StreamService::validate_config`; the inline checks only short-circuit
 * the network roundtrip with a user-visible error.
 */
export function OutputGroupModal({ open, onClose, mode, group }: OutputGroupModalProps) {
  const { t } = useTranslation();
  const { addOutputGroup, updateOutputGroup } = useProfileStore();
  const form = useFormState<FormData>(defaultFormData);
  const formData = form.values;
  const [saving, setSaving] = useState(false);
  const [serverError, setServerError] = useState<string | undefined>();
  const [encoders, setEncoders] = useState<Encoders>({
    video: ['libx264'],
    audio: ['aac'],
    metadata: {},
  });
  const [loadingEncoders, setLoadingEncoders] = useState(false);

  const isDefaultGroup = mode === 'edit' && group?.isDefault === true;

  useEffect(() => {
    if (isDefaultGroup && open) {
      onClose();
    }
  }, [isDefaultGroup, open, onClose]);

  // Load available encoders when modal opens; seed codecs in create mode.
  useEffect(() => {
    if (open) {
      setLoadingEncoders(true);
      api.system
        .getEncoders()
        .then((enc) => {
          setEncoders(enc);
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
    // `form.merge` is intentionally outside the dep array — seeding only
    // runs on the create-mode open transition, not on every field edit.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, mode]);

  useEffect(() => {
    if (open) {
      if (mode === 'edit' && group) {
        const videoBitrate = group.video.bitrate.replace(/[^\d]/g, '') || '6000';
        const resolution = `${group.video.width}x${group.video.height}`;

        form.reset({
          name: group.name || '',
          generatePts: group.generatePts !== false,
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

  const rules: Partial<Record<keyof FormData, ValidationRule<FormData>>> = {
    name: (v) => (!v.name.trim() ? t('validation.outputGroupNameRequired') : null),
    videoBitrate: (v) => {
      const bitrate = parseInt(v.videoBitrate);
      if (
        isNaN(bitrate) ||
        bitrate < clientConfig.BITRATE_MIN ||
        bitrate > clientConfig.BITRATE_MAX
      ) {
        return t('validation.bitrateRange');
      }
      return null;
    },
    keyframeIntervalSeconds: (v) => {
      if (!v.keyframeIntervalSeconds.trim()) return null;
      const interval = Number(v.keyframeIntervalSeconds);
      if (
        !Number.isFinite(interval) ||
        !Number.isInteger(interval) ||
        interval < clientConfig.KEYFRAME_MIN ||
        interval > clientConfig.KEYFRAME_MAX
      ) {
        return t('errors.invalidInput', { defaultValue: 'Invalid input' });
      }
      return null;
    },
  };
  const { errors, validate, clear: clearErrors } = useFormValidation<FormData>(formData, rules);

  const handleVideoChange = useCallback(
    <K extends keyof VideoFormValues>(field: K, value: VideoFormValues[K]): void => {
      form.set(field, value as FormData[K]);
    },
    [form]
  );

  const handleAudioChange = useCallback(
    <K extends keyof AudioFormValues>(field: K, value: AudioFormValues[K]): void => {
      form.set(field, value as FormData[K]);
    },
    [form]
  );

  const handleContainerChange = useCallback(
    <K extends keyof ContainerFormValues>(field: K, value: ContainerFormValues[K]): void => {
      form.set(field, value as FormData[K]);
    },
    [form]
  );

  const handleSave = async (): Promise<void> => {
    if (!validate()) return;
    setServerError(undefined);

    setSaving(true);
    try {
      const [width, height] = formData.resolution.split('x').map(Number);
      const presetSupported = isPresetSupportedFor(formData.videoCodec);

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

      const audio: AudioSettings = {
        codec: formData.audioCodec,
        bitrate: formData.audioBitrate,
        channels: parseInt(formData.audioChannels),
        sampleRate: parseInt(formData.audioSampleRate),
      };

      const container: ContainerSettings = {
        format: formData.containerFormat,
      };

      const groupData: OutputGroup = {
        id: mode === 'edit' && group ? group.id : crypto.randomUUID(),
        // Trim to match the validator's `!v.name.trim()` rule — without
        // this, leading/trailing whitespace survives into the persisted
        // group and the visible name disagrees with what the user typed.
        name: formData.name.trim(),
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
      onClose();
    } catch (error) {
      setServerError(String(error));
    } finally {
      setSaving(false);
    }
  };

  const title = mode === 'create' ? t('modals.createOutputGroup') : t('modals.editOutputGroup');

  if (isDefaultGroup) {
    return null;
  }

  const videoValues: VideoFormValues = {
    videoCodec: formData.videoCodec,
    resolution: formData.resolution,
    fps: formData.fps,
    videoBitrate: formData.videoBitrate,
    preset: formData.preset,
    profile: formData.profile,
    keyframeIntervalSeconds: formData.keyframeIntervalSeconds,
  };
  const videoErrors: VideoFormErrors = {
    videoBitrate: errors.videoBitrate,
    keyframeIntervalSeconds: errors.keyframeIntervalSeconds,
  };
  const audioValues: AudioFormValues = {
    audioCodec: formData.audioCodec,
    audioBitrate: formData.audioBitrate,
    audioChannels: formData.audioChannels,
    audioSampleRate: formData.audioSampleRate,
  };
  const containerValues: ContainerFormValues = {
    containerFormat: formData.containerFormat,
  };

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
            {(() => {
              if (saving) return t('common.saving');
              if (mode === 'create') return t('modals.createGroup');
              return t('common.saveChanges');
            })()}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-4">
        {mode === 'create' && (
          <div className="p-3 bg-primary-muted rounded-lg text-sm text-text-secondary leading-normal">
            {t('modals.outputGroupExplanation', {
              defaultValue:
                'Custom output groups re-encode your incoming stream to different settings. Use these when you need to send different quality streams to different platforms. The default passthrough group relays your stream as-is without re-encoding.',
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
          onChange={(e) => form.set('name', e.target.value)}
          error={errors.name}
        />

        <div className="p-3 bg-bg-muted rounded-lg">
          <Toggle
            checked={formData.generatePts}
            onChange={(checked) => form.set('generatePts', checked)}
            label={t('encoder.generatePts')}
            description={t('encoder.generatePtsDescription')}
          />
        </div>

        <VideoSettingsForm
          values={videoValues}
          errors={videoErrors}
          encoders={encoders}
          loadingEncoders={loadingEncoders}
          onChange={handleVideoChange}
        />

        <AudioSettingsForm
          values={audioValues}
          encoders={encoders}
          loadingEncoders={loadingEncoders}
          onChange={handleAudioChange}
        />

        <ContainerSettingsForm values={containerValues} onChange={handleContainerChange} />
      </div>
    </Modal>
  );
}
