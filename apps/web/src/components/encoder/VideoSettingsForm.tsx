import { useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Input } from '@/components/ui/Input';
import { Select, SelectOption } from '@/components/ui/Select';
import { encoderPresets, getPresetValues, getDefaultPreset } from '@/lib/encoderPresets';
import type { Encoders } from '@/types/stream';

const ENCODER_DEFAULT_LABELS: Record<string, string> = {
  h264_vaapi: 'VAAPI (Linux)',
  hevc_vaapi: 'VAAPI HEVC (Linux)',
  av1_vaapi: 'VAAPI AV1 (Linux)',
};

export interface VideoFormValues {
  videoCodec: string;
  resolution: string;
  fps: string;
  videoBitrate: string;
  preset: string;
  profile: string;
  keyframeIntervalSeconds: string;
}

export interface VideoFormErrors {
  videoBitrate?: string;
  keyframeIntervalSeconds?: string;
}

interface VideoSettingsFormProps {
  values: VideoFormValues;
  errors: VideoFormErrors;
  encoders: Encoders;
  loadingEncoders: boolean;
  onChange: <K extends keyof VideoFormValues>(field: K, value: VideoFormValues[K]) => void;
}

export function VideoSettingsForm({
  values,
  errors,
  encoders,
  loadingEncoders,
  onChange,
}: VideoSettingsFormProps): React.ReactElement {
  const { t } = useTranslation();
  const tDynamic = t as (
    key: string,
    options?: { defaultValue?: string; [key: string]: string | number | undefined }
  ) => string;

  const presetValues = useMemo(() => getPresetValues(values.videoCodec), [values.videoCodec]);
  const presetSupported = presetValues.length > 0;

  const videoCodecOptions: SelectOption[] = useMemo(
    () =>
      encoders.video.map((enc) => {
        const defaultLabel = ENCODER_DEFAULT_LABELS[enc] || enc;
        const label = tDynamic(`encoder.encoders.${enc}`, { defaultValue: defaultLabel });
        return { value: enc, label };
      }),
    [encoders.video, tDynamic]
  );

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

  // Keep `preset` consistent with the selected encoder. Codecs that
  // don't expose presets (e.g. VAAPI) clear the field; codecs that do
  // fall back to their default when the current value is invalid.
  useEffect(() => {
    if (!presetSupported) {
      if (values.preset) onChange('preset', '');
      return;
    }
    if (!presetValues.includes(values.preset)) {
      onChange('preset', getDefaultPreset(values.videoCodec, presetValues));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [presetSupported, presetValues, values.preset, values.videoCodec]);

  return (
    <div className="p-3 bg-bg-muted rounded-lg">
      <div className="mb-3 text-sm font-medium text-text-primary">{t('modals.videoSettings')}</div>

      <div className="grid grid-cols-2 gap-3 mb-3">
        <Select
          label={t('encoder.videoEncoder')}
          value={values.videoCodec}
          onChange={(e) => onChange('videoCodec', e.target.value)}
          options={videoCodecOptions}
          disabled={loadingEncoders}
        />
        <Select
          label={t('encoder.resolution')}
          value={values.resolution}
          onChange={(e) => onChange('resolution', e.target.value)}
          options={resolutionOptions}
        />
      </div>

      <div className="grid grid-cols-3 gap-3 mb-3">
        <Select
          label={t('encoder.frameRate')}
          value={values.fps}
          onChange={(e) => onChange('fps', e.target.value)}
          options={fpsOptions}
        />
        <Input
          label={t('encoder.videoBitrate')}
          type="number"
          placeholder={t('modals.videoBitratePlaceholder')}
          value={values.videoBitrate}
          onChange={(e) => onChange('videoBitrate', e.target.value)}
          error={errors.videoBitrate}
        />
        <Select
          label={t('encoder.profile')}
          value={values.profile}
          onChange={(e) => onChange('profile', e.target.value)}
          options={profileOptions}
        />
      </div>

      <Select
        label={t('encoder.preset')}
        value={values.preset}
        onChange={(e) => onChange('preset', e.target.value)}
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
        value={values.keyframeIntervalSeconds}
        onChange={(e) => onChange('keyframeIntervalSeconds', e.target.value)}
        helper={t('encoder.keyframeIntervalHelper')}
        error={errors.keyframeIntervalSeconds}
      />
    </div>
  );
}

export function isPresetSupportedFor(codec: string): boolean {
  return getPresetValues(codec).length > 0;
}
