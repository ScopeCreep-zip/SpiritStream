import React from 'react';
import { useTranslation } from 'react-i18next';
import { RefreshCw, Mic } from 'lucide-react';
import { Input } from '@/components/ui/Input';
import { Select, SelectOption } from '@/components/ui/Select';
import { Button } from '@/components/ui/Button';
import { Toggle } from '@/components/ui/Toggle';
import type { CameraSource } from '@/types/source';
import type { DeviceDiscoveryState } from '@/stores/sourceStore';

interface CameraSourceFormProps {
  data: CameraSource;
  onChange: (data: CameraSource) => void;
  devices: DeviceDiscoveryState;
  onRefreshDevices: () => void;
}

export const CameraSourceForm = React.memo(({ data, onChange, devices, onRefreshDevices }: CameraSourceFormProps) => {
  const { t } = useTranslation();

  const cameraOptions: SelectOption[] = devices.cameras.map((c) => ({
    value: c.deviceId,
    label: c.name,
  }));

  const selectedCamera = devices.cameras.find(c => c.deviceId === data.deviceId);
  const hasLinkedAudio = selectedCamera?.linkedAudioDeviceId;

  return (
    <div className="flex flex-col gap-4">
      <Input
        label={t('stream.sourceName', { defaultValue: 'Source Name' })}
        value={data.name}
        onChange={(e) => onChange({ ...data, name: e.target.value })}
        placeholder="Camera"
      />
      <div className="flex items-center gap-2">
        <div className="flex-1">
          <Select
            label={t('stream.camera', { defaultValue: 'Camera Device' })}
            value={data.deviceId}
            onChange={(e) => {
              const deviceId = e.target.value;
              const camera = devices.cameras.find((c) => c.deviceId === deviceId);
              onChange({
                ...data,
                deviceId,
                name: data.name || camera?.name || 'Camera',
                linkedAudioDeviceId: camera?.linkedAudioDeviceId,
              });
            }}
            options={cameraOptions}
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
      <div className="grid grid-cols-2 sm:grid-cols-3 gap-3">
        <Input
          label={t('stream.width', { defaultValue: 'Width' })}
          type="number"
          value={data.width !== undefined ? String(data.width) : ''}
          onChange={(e) => onChange({ ...data, width: e.target.value ? parseInt(e.target.value) : undefined })}
          placeholder="1920"
        />
        <Input
          label={t('stream.height', { defaultValue: 'Height' })}
          type="number"
          value={data.height !== undefined ? String(data.height) : ''}
          onChange={(e) => onChange({ ...data, height: e.target.value ? parseInt(e.target.value) : undefined })}
          placeholder="1080"
        />
        <Input
          label={t('stream.fps', { defaultValue: 'FPS' })}
          type="number"
          value={data.fps !== undefined ? String(data.fps) : ''}
          onChange={(e) => onChange({ ...data, fps: e.target.value ? parseInt(e.target.value) : undefined })}
          placeholder="30"
        />
      </div>
      <p className="text-xs text-muted">
        {t('stream.cameraResolutionHelper', { defaultValue: 'Leave blank to use device defaults' })}
      </p>

      {hasLinkedAudio ? (
        <div className="flex items-center justify-between">
          <div>
            <span className="text-sm">{t('stream.captureAudio', { defaultValue: 'Capture Audio' })}</span>
            {data.captureAudio && (
              <p className="text-xs text-muted mt-0.5">
                <Mic className="w-3 h-3 inline mr-1" />
                {selectedCamera?.linkedAudioDeviceName}
              </p>
            )}
          </div>
          <Toggle
            checked={data.captureAudio}
            onChange={(checked) => onChange({
              ...data,
              captureAudio: checked,
              linkedAudioDeviceId: checked ? selectedCamera?.linkedAudioDeviceId : undefined,
            })}
          />
        </div>
      ) : data.deviceId ? (
        <div className="p-3 bg-[var(--bg-sunken)] rounded-lg text-sm text-muted">
          <p>{t('stream.noLinkedMic', { defaultValue: 'No microphone detected for this camera.' })}</p>
          <p className="text-xs mt-1">
            {t('stream.noLinkedMicHelper', { defaultValue: 'You can add a separate Audio Device source manually.' })}
          </p>
        </div>
      ) : null}
    </div>
  );
});

CameraSourceForm.displayName = 'CameraSourceForm';
