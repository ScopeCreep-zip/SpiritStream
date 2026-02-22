import React from 'react';
import { useTranslation } from 'react-i18next';
import { RefreshCw } from 'lucide-react';
import { Input } from '@/components/ui/Input';
import { Select, SelectOption } from '@/components/ui/Select';
import { Button } from '@/components/ui/Button';
import { Toggle } from '@/components/ui/Toggle';
import type { ScreenCaptureSource, DisplayInfo } from '@/types/source';
import type { DeviceDiscoveryState } from '@/stores/sourceStore';

interface ScreenCaptureFormProps {
  data: ScreenCaptureSource;
  onChange: (data: ScreenCaptureSource) => void;
  devices: DeviceDiscoveryState;
  onRefreshDevices: () => void;
}

export const ScreenCaptureForm = React.memo(({ data, onChange, devices, onRefreshDevices }: ScreenCaptureFormProps) => {
  const { t } = useTranslation();

  const displayOptions: SelectOption[] = devices.displays.map((d: DisplayInfo) => ({
    value: d.displayId,
    label: `${d.name} (${d.width}x${d.height})${d.isPrimary ? ' - Primary' : ''}`,
  }));

  const resolutionOptions: SelectOption[] = [
    { value: '720p', label: '720p (1280x720) — Low resource usage' },
    { value: '1080p', label: '1080p (1920x1080) — Recommended' },
    { value: '1440p', label: '1440p (2560x1440)' },
    { value: '2160p', label: '4K (3840x2160)' },
    { value: 'captured', label: 'Native — Full display resolution' },
  ];

  return (
    <div className="flex flex-col gap-4">
      <Input
        label={t('stream.sourceName', { defaultValue: 'Source Name' })}
        value={data.name}
        onChange={(e) => onChange({ ...data, name: e.target.value })}
        placeholder="Screen Capture"
      />
      <div className="flex items-center gap-2">
        <div className="flex-1">
          <Select
            label={t('stream.display', { defaultValue: 'Display' })}
            value={data.displayId}
            onChange={(e) => {
              const selectedDisplay = devices.displays.find((d: DisplayInfo) => d.displayId === e.target.value);
              onChange({
                ...data,
                displayId: e.target.value,
                deviceName: selectedDisplay?.deviceName,
                name: data.name || selectedDisplay?.name || '',
              });
            }}
            options={displayOptions}
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
      <Input
        label={t('stream.fps', { defaultValue: 'Frame Rate' })}
        type="number"
        value={String(data.fps)}
        onChange={(e) => onChange({ ...data, fps: parseInt(e.target.value) || 30 })}
      />
      <Select
        label={t('stream.captureResolution', { defaultValue: 'Capture Resolution' })}
        value={data.captureResolution || '1080p'}
        onChange={(e) => onChange({ ...data, captureResolution: e.target.value as ScreenCaptureSource['captureResolution'] })}
        options={resolutionOptions}
      />
      <div className="flex items-center justify-between">
        <span className="text-sm">{t('stream.captureCursor', { defaultValue: 'Capture Cursor' })}</span>
        <Toggle
          checked={data.captureCursor}
          onChange={(checked) => onChange({ ...data, captureCursor: checked })}
        />
      </div>
      <div className="flex items-center justify-between">
        <span className="text-sm">{t('stream.captureAudio', { defaultValue: 'Capture Desktop Audio' })}</span>
        <Toggle
          checked={data.captureAudio}
          onChange={(checked) => onChange({ ...data, captureAudio: checked })}
        />
      </div>
    </div>
  );
});

ScreenCaptureForm.displayName = 'ScreenCaptureForm';
