/**
 * Audio Device Source Form
 * Configuration form for microphone/audio input sources
 */
import { useTranslation } from 'react-i18next';
import { RefreshCw } from 'lucide-react';
import { Input } from '@/components/ui/Input';
import { Select, SelectOption } from '@/components/ui/Select';
import { Button } from '@/components/ui/Button';
import type { AudioDeviceSource, AudioInputDevice } from '@/types/source';
import type { SourceFormProps } from './types';

export interface AudioDeviceSourceFormProps extends SourceFormProps<AudioDeviceSource> {
  audioDevices: AudioInputDevice[];
}

export function AudioDeviceSourceForm({
  data,
  onChange,
  audioDevices,
  isDiscovering,
  onRefreshDevices,
}: AudioDeviceSourceFormProps) {
  const { t } = useTranslation();

  const deviceOptions: SelectOption[] = audioDevices.map((d) => ({
    value: d.deviceId,
    label: `${d.name}${d.isDefault ? ' (Default)' : ''}`,
  }));

  return (
    <div className="flex flex-col gap-4">
      <Input
        label={t('stream.sourceName', { defaultValue: 'Source Name' })}
        value={data.name}
        onChange={(e) => onChange({ ...data, name: e.target.value })}
        placeholder="Audio Device"
      />
      <div className="flex items-center gap-2">
        <div className="flex-1">
          <Select
            label={t('stream.audioDevice', { defaultValue: 'Audio Device' })}
            value={data.deviceId}
            onChange={(e) => {
              const deviceId = e.target.value;
              const device = audioDevices.find((d) => d.deviceId === deviceId);
              onChange({
                ...data,
                deviceId,
                name: data.name || device?.name || 'Audio Device',
              });
            }}
            options={deviceOptions}
            disabled={isDiscovering}
          />
        </div>
        {onRefreshDevices && (
          <div className="flex items-end">
            <Button
              variant="ghost"
              className={`h-10 ${isDiscovering ? 'opacity-60' : ''}`}
              onClick={onRefreshDevices}
              disabled={isDiscovering}
              title={t('common.refresh', { defaultValue: 'Refresh' })}
            >
              <RefreshCw className={`w-4 h-4 ${isDiscovering ? 'animate-spin' : ''}`} />
            </Button>
          </div>
        )}
      </div>
      <div className="grid grid-cols-2 gap-4">
        <Input
          label={t('stream.channels', { defaultValue: 'Channels' })}
          type="number"
          value={String(data.channels || 2)}
          onChange={(e) => onChange({ ...data, channels: parseInt(e.target.value) || 2 })}
          min={1}
          max={8}
        />
        <Input
          label={t('stream.sampleRate', { defaultValue: 'Sample Rate' })}
          type="number"
          value={String(data.sampleRate || 48000)}
          onChange={(e) => onChange({ ...data, sampleRate: parseInt(e.target.value) || 48000 })}
        />
      </div>
    </div>
  );
}
