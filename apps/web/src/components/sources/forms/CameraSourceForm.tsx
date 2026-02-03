/**
 * Camera Source Form
 * Configuration form for webcam/camera sources
 */
import { useTranslation } from 'react-i18next';
import { RefreshCw, Link } from 'lucide-react';
import { Input } from '@/components/ui/Input';
import { Select, SelectOption } from '@/components/ui/Select';
import { Button } from '@/components/ui/Button';
import { Toggle } from '@/components/ui/Toggle';
import type { CameraSource, CameraDevice } from '@/types/source';
import type { SourceFormProps } from './types';

export interface CameraSourceFormProps extends SourceFormProps<CameraSource> {
  cameras: CameraDevice[];
  /** Called when user wants to add linked audio source */
  onAddLinkedAudio?: (cameraName: string, audioDeviceId: string, audioDeviceName: string) => void;
}

export function CameraSourceForm({
  data,
  onChange,
  cameras,
  isDiscovering,
  onRefreshDevices,
  onAddLinkedAudio,
}: CameraSourceFormProps) {
  const { t } = useTranslation();

  const cameraOptions: SelectOption[] = cameras.map((c) => ({
    value: c.deviceId,
    label: c.name,
  }));

  const selectedCamera = cameras.find(c => c.deviceId === data.deviceId);
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
              const camera = cameras.find((c) => c.deviceId === deviceId);
              onChange({
                ...data,
                deviceId,
                name: data.name || camera?.name || 'Camera',
                linkedAudioDeviceId: camera?.linkedAudioDeviceId,
              });
            }}
            options={cameraOptions}
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

      <div className="flex items-center justify-between">
        <span className="text-sm">{t('stream.captureAudio', { defaultValue: 'Capture Audio' })}</span>
        <Toggle
          checked={data.captureAudio}
          onChange={(checked) => onChange({ ...data, captureAudio: checked })}
        />
      </div>

      {/* Linked audio device info */}
      {hasLinkedAudio && selectedCamera && (
        <div className="p-3 bg-[var(--bg-elevated)] rounded-lg border border-[var(--border)]">
          <div className="flex items-center gap-2 text-sm">
            <Link className="w-4 h-4 text-[var(--primary)]" />
            <span className="text-muted">
              {t('stream.linkedMicrophone', { defaultValue: 'Linked microphone:' })}
            </span>
            <span className="font-medium">{selectedCamera.linkedAudioDeviceName || 'Built-in Microphone'}</span>
          </div>
          {onAddLinkedAudio && (
            <Button
              variant="ghost"
              size="sm"
              className="mt-2"
              onClick={() => onAddLinkedAudio(
                data.name,
                selectedCamera.linkedAudioDeviceId!,
                selectedCamera.linkedAudioDeviceName || 'Microphone'
              )}
            >
              {t('stream.addLinkedAudio', { defaultValue: 'Add as Audio Source' })}
            </Button>
          )}
        </div>
      )}
    </div>
  );
}
