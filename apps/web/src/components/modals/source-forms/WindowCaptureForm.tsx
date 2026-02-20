import React from 'react';
import { useTranslation } from 'react-i18next';
import { RefreshCw } from 'lucide-react';
import { Input } from '@/components/ui/Input';
import { Select, SelectOption } from '@/components/ui/Select';
import { Button } from '@/components/ui/Button';
import { Toggle } from '@/components/ui/Toggle';
import type { WindowCaptureSource, WindowInfo } from '@/types/source';
import type { DeviceDiscoveryState } from '@/stores/sourceStore';

interface WindowCaptureFormProps {
  data: WindowCaptureSource;
  onChange: (data: WindowCaptureSource) => void;
  devices: DeviceDiscoveryState;
  onRefreshWindows: () => void;
}

export const WindowCaptureForm = React.memo(({ data, onChange, devices, onRefreshWindows }: WindowCaptureFormProps) => {
  const { t } = useTranslation();

  const windowOptions: SelectOption[] = devices.windows.map((w: WindowInfo) => ({
    value: w.windowId,
    label: w.title || w.processName || 'Unknown Window',
  }));

  return (
    <div className="flex flex-col gap-4">
      <Input
        label={t('stream.sourceName', { defaultValue: 'Source Name' })}
        value={data.name}
        onChange={(e) => onChange({ ...data, name: e.target.value })}
        placeholder="Window Capture"
      />
      <div className="flex items-center gap-2">
        <div className="flex-1">
          <Select
            label={t('stream.window', { defaultValue: 'Window' })}
            value={data.windowId}
            onChange={(e) => {
              const windowId = e.target.value;
              const window = devices.windows.find((w: WindowInfo) => w.windowId === windowId);
              onChange({
                ...data,
                windowId,
                windowTitle: window?.title || '',
                processName: window?.processName,
                name: data.name || window?.title || 'Window Capture',
              });
            }}
            options={windowOptions}
            disabled={devices.isDiscovering}
          />
        </div>
        <div className="flex items-end">
          <Button
            variant="ghost"
            className={`h-10 ${devices.isDiscovering ? 'opacity-60' : ''}`}
            onClick={onRefreshWindows}
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
      <div className="flex items-center justify-between">
        <span className="text-sm">{t('stream.captureCursor', { defaultValue: 'Capture Cursor' })}</span>
        <Toggle
          checked={data.captureCursor}
          onChange={(checked) => onChange({ ...data, captureCursor: checked })}
        />
      </div>
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

WindowCaptureForm.displayName = 'WindowCaptureForm';
