/**
 * Screen Capture Source Form
 * Configuration form for display capture sources
 */
import { useTranslation } from 'react-i18next';
import { RefreshCw } from 'lucide-react';
import { Input } from '@/components/ui/Input';
import { Select, SelectOption } from '@/components/ui/Select';
import { Button } from '@/components/ui/Button';
import { Toggle } from '@/components/ui/Toggle';
import type { ScreenCaptureSource, DisplayInfo } from '@/types/source';
import type { SourceFormProps } from './types';

export interface ScreenCaptureSourceFormProps extends SourceFormProps<ScreenCaptureSource> {
  displays: DisplayInfo[];
}

export function ScreenCaptureSourceForm({
  data,
  onChange,
  displays,
  isDiscovering,
  onRefreshDevices,
}: ScreenCaptureSourceFormProps) {
  const { t } = useTranslation();

  const displayOptions: SelectOption[] = displays.map((d) => ({
    value: d.displayId,
    label: `${d.name} (${d.width}x${d.height})${d.isPrimary ? ' - Primary' : ''}`,
  }));

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
              const selectedDisplay = displays.find(d => d.displayId === e.target.value);
              onChange({
                ...data,
                displayId: e.target.value,
                deviceName: selectedDisplay?.deviceName,
                name: data.name || selectedDisplay?.name || '',
              });
            }}
            options={displayOptions}
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
        <span className="text-sm">{t('stream.captureAudio', { defaultValue: 'Capture Desktop Audio' })}</span>
        <Toggle
          checked={data.captureAudio}
          onChange={(checked) => onChange({ ...data, captureAudio: checked })}
        />
      </div>
    </div>
  );
}
