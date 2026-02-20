import React from 'react';
import { useTranslation } from 'react-i18next';
import { RefreshCw } from 'lucide-react';
import { Input } from '@/components/ui/Input';
import { Select, SelectOption } from '@/components/ui/Select';
import { Button } from '@/components/ui/Button';
import { Toggle } from '@/components/ui/Toggle';
import type { CaptureCardSource } from '@/types/source';
import type { DeviceDiscoveryState } from '@/stores/sourceStore';

interface CaptureCardFormProps {
  data: CaptureCardSource;
  onChange: (data: CaptureCardSource) => void;
  devices: DeviceDiscoveryState;
  onRefreshDevices: () => void;
}

export const CaptureCardForm = React.memo(({ data, onChange, devices, onRefreshDevices }: CaptureCardFormProps) => {
  const { t } = useTranslation();

  const captureCardOptions: SelectOption[] = devices.captureCards.map((c) => ({
    value: c.deviceId,
    label: c.name,
  }));

  const inputFormatOptions: SelectOption[] = [
    { value: '', label: t('common.auto', { defaultValue: 'Auto' }) },
    { value: 'hdmi', label: 'HDMI' },
    { value: 'component', label: 'Component' },
    { value: 'sdi', label: 'SDI' },
  ];

  return (
    <div className="flex flex-col gap-4">
      <Input
        label={t('stream.sourceName', { defaultValue: 'Source Name' })}
        value={data.name}
        onChange={(e) => onChange({ ...data, name: e.target.value })}
        placeholder="Capture Card"
      />
      <div className="flex items-center gap-2">
        <div className="flex-1">
          <Select
            label={t('stream.captureCard', { defaultValue: 'Capture Card' })}
            value={data.deviceId}
            onChange={(e) => {
              const deviceId = e.target.value;
              const card = devices.captureCards.find((c) => c.deviceId === deviceId);
              onChange({
                ...data,
                deviceId,
                name: data.name || card?.name || 'Capture Card',
              });
            }}
            options={captureCardOptions}
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
      <Select
        label={t('stream.inputFormat', { defaultValue: 'Input Format' })}
        value={data.inputFormat || ''}
        onChange={(e) => onChange({ ...data, inputFormat: e.target.value || undefined })}
        options={inputFormatOptions}
      />
      <div className="flex items-center justify-between">
        <span className="text-sm">{t('stream.captureAudio', { defaultValue: 'Capture Audio' })}</span>
        <Toggle
          checked={data.captureAudio}
          onChange={(checked) => onChange({ ...data, captureAudio: checked })}
        />
      </div>
    </div>
  );
});

CaptureCardForm.displayName = 'CaptureCardForm';
