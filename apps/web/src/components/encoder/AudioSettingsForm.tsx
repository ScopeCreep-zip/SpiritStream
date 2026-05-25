import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Select, SelectOption } from '@/components/ui/Select';
import { encoderPresets } from '@/lib/encoderPresets';
import type { Encoders } from '@/types/stream';

export interface AudioFormValues {
  audioCodec: string;
  audioBitrate: string;
  audioChannels: string;
  audioSampleRate: string;
}

interface AudioSettingsFormProps {
  values: AudioFormValues;
  encoders: Encoders;
  loadingEncoders: boolean;
  onChange: <K extends keyof AudioFormValues>(field: K, value: AudioFormValues[K]) => void;
}

export function AudioSettingsForm({
  values,
  encoders,
  loadingEncoders,
  onChange,
}: AudioSettingsFormProps): React.ReactElement {
  const { t } = useTranslation();
  const tDynamic = t as (
    key: string,
    options?: { defaultValue?: string; [key: string]: string | number | undefined },
  ) => string;

  const audioCodecOptions: SelectOption[] = useMemo(
    () =>
      encoders.audio.map((enc) => {
        const label = tDynamic(`audio.codecs.${enc}`, { defaultValue: enc });
        return { value: enc, label };
      }),
    [encoders.audio, tDynamic],
  );

  const audioBitrateOptions: SelectOption[] = useMemo(
    () =>
      encoderPresets.AUDIO_BITRATE_VALUES.map((value) => ({
        value,
        label: tDynamic(`audio.bitrates.${value}`, { defaultValue: value }),
      })),
    [tDynamic],
  );

  const audioChannelsOptions: SelectOption[] = useMemo(
    () =>
      encoderPresets.AUDIO_CHANNELS_VALUES.map((value) => {
        if (value === '1') {
          return { value, label: tDynamic('audio.channels.mono', { defaultValue: 'Mono' }) };
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
    [tDynamic],
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
    [tDynamic],
  );

  return (
    <div className="p-3 bg-bg-muted rounded-lg">
      <div className="mb-3 text-sm font-medium text-text-primary">
        {t('modals.audioSettings')}
      </div>

      <div className="grid grid-cols-2 gap-3 mb-3">
        <Select
          label={t('modals.audioCodec')}
          value={values.audioCodec}
          onChange={(e) => onChange('audioCodec', e.target.value)}
          options={audioCodecOptions}
          disabled={loadingEncoders}
        />
        <Select
          label={t('modals.audioBitrate')}
          value={values.audioBitrate}
          onChange={(e) => onChange('audioBitrate', e.target.value)}
          options={audioBitrateOptions}
        />
      </div>

      <div className="grid grid-cols-2 gap-3">
        <Select
          label={t('modals.audioChannels')}
          value={values.audioChannels}
          onChange={(e) => onChange('audioChannels', e.target.value)}
          options={audioChannelsOptions}
        />
        <Select
          label={t('modals.audioSampleRate')}
          value={values.audioSampleRate}
          onChange={(e) => onChange('audioSampleRate', e.target.value)}
          options={audioSampleRateOptions}
        />
      </div>
    </div>
  );
}
