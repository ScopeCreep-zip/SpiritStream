import React from 'react';
import { useTranslation } from 'react-i18next';
import { RefreshCw } from 'lucide-react';
import { Input } from '@/components/ui/Input';
import { Select, SelectOption } from '@/components/ui/Select';
import { Button } from '@/components/ui/Button';
import type { AudioDeviceSource } from '@/types/source';
import type { DeviceDiscoveryState } from '@/stores/sourceStore';

interface AudioDeviceFormProps {
  data: AudioDeviceSource;
  onChange: (data: AudioDeviceSource) => void;
  devices: DeviceDiscoveryState;
  onRefreshDevices: () => void;
}

export const AudioDeviceForm = React.memo(({ data, onChange, devices, onRefreshDevices }: AudioDeviceFormProps) => {
  const { t } = useTranslation();

  const audioDeviceOptions: SelectOption[] = devices.audioDevices.map((d) => ({
    value: d.deviceId,
    label: `${d.name}${d.isDefault ? ' (Default)' : ''}`,
  }));

  return (
    <div className="flex flex-col gap-4">
      <Input
        label={t('stream.sourceName', { defaultValue: 'Source Name' })}
        value={data.name}
        onChange={(e) => onChange({ ...data, name: e.target.value })}
        placeholder="Microphone"
      />
      <div className="flex items-center gap-2">
        <div className="flex-1">
          <Select
            label={t('stream.audioDevice', { defaultValue: 'Audio Device' })}
            value={data.deviceId}
            onChange={(e) => {
              const deviceId = e.target.value;
              const device = devices.audioDevices.find((d) => d.deviceId === deviceId);
              onChange({
                ...data,
                deviceId,
                name: data.name || device?.name || 'Audio Device',
              });
            }}
            options={audioDeviceOptions}
            disabled={devices.isDiscovering}
          />
        </div>
        <div className="flex items-end">
          <Button
            variant="ghost"
            className={`h-10 ${devices.isDiscovering ? 'opacity-60' : ''}`}
            onClick={onRefreshDevices}
            disabled={devices.isDiscovering}
            title={t('common.refresh', { defaultValue: 'Refresh' })}
          >
            <RefreshCw className={`w-4 h-4 ${devices.isDiscovering ? 'animate-spin' : ''}`} />
          </Button>
        </div>
      </div>
      <div className="grid grid-cols-2 gap-3">
        <Input
          label={t('stream.channels', { defaultValue: 'Channels' })}
          type="number"
          value={data.channels !== undefined ? String(data.channels) : ''}
          onChange={(e) => onChange({ ...data, channels: e.target.value ? parseInt(e.target.value) : undefined })}
          placeholder="2"
        />
        <Input
          label={t('stream.sampleRate', { defaultValue: 'Sample Rate (Hz)' })}
          type="number"
          value={data.sampleRate !== undefined ? String(data.sampleRate) : ''}
          onChange={(e) => onChange({ ...data, sampleRate: e.target.value ? parseInt(e.target.value) : undefined })}
          placeholder="48000"
        />
      </div>
      <p className="text-xs text-muted">
        {t('stream.audioDeviceHelper', { defaultValue: 'Leave blank to use device defaults' })}
      </p>
    </div>
  );
});

AudioDeviceForm.displayName = 'AudioDeviceForm';
