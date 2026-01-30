/**
 * Add Source Modal
 * Modal for adding new input sources to a profile
 */
import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Radio,
  Film,
  Monitor,
  Camera,
  Usb,
  Mic,
  ArrowLeft,
  FolderOpen,
  RefreshCw,
  Palette,
  Type,
  Globe,
} from 'lucide-react';
import { Modal } from '@/components/ui/Modal';
import { Input } from '@/components/ui/Input';
import { Select, SelectOption } from '@/components/ui/Select';
import { Button } from '@/components/ui/Button';
import { Toggle } from '@/components/ui/Toggle';
import { useProfileStore } from '@/stores/profileStore';
import { useSourceStore } from '@/stores/sourceStore';
import { usePermissionCheck, type SourcePermissionType } from '@/stores/permissionStore';
import { dialogs } from '@/lib/backend/dialogs';
import { backendMode } from '@/lib/backend/env';
import { useFileBrowser } from '@/hooks/useFileBrowser';
import type {
  SourceType,
  Source,
  RtmpSource,
  MediaFileSource,
  ScreenCaptureSource,
  CameraSource,
  CaptureCardSource,
  AudioDeviceSource,
  ColorSource,
  TextSource,
  BrowserSource,
} from '@/types/source';
import {
  createDefaultRtmpSource,
  createDefaultMediaFileSource,
  createDefaultScreenCaptureSource,
  createDefaultCameraSource,
  createDefaultCaptureCardSource,
  createDefaultAudioDeviceSource,
  createDefaultColorSource,
  createDefaultTextSource,
  createDefaultBrowserSource,
  getSourceTypeLabel,
} from '@/types/source';

export interface AddSourceModalProps {
  open: boolean;
  onClose: () => void;
  profileName: string;
  /** If provided, skip type selection and go directly to configuring this source type */
  filterType?: SourceType;
  /** If provided, hide these source types from the selection */
  excludeTypes?: SourceType[];
  /** Called after a source is successfully added, with the new source */
  onSourceAdded?: (source: Source) => void;
}

type ModalStep = 'select-type' | 'configure';

const SOURCE_TYPES: { type: SourceType; icon: React.ReactNode }[] = [
  { type: 'rtmp', icon: <Radio className="w-5 h-5" /> },
  { type: 'mediaFile', icon: <Film className="w-5 h-5" /> },
  { type: 'screenCapture', icon: <Monitor className="w-5 h-5" /> },
  { type: 'camera', icon: <Camera className="w-5 h-5" /> },
  { type: 'captureCard', icon: <Usb className="w-5 h-5" /> },
  { type: 'audioDevice', icon: <Mic className="w-5 h-5" /> },
  { type: 'color', icon: <Palette className="w-5 h-5" /> },
  { type: 'text', icon: <Type className="w-5 h-5" /> },
  { type: 'browser', icon: <Globe className="w-5 h-5" /> },
];

export function AddSourceModal({ open, onClose, profileName, filterType, excludeTypes = [], onSourceAdded }: AddSourceModalProps) {
  const { t } = useTranslation();
  const { setCurrentSources } = useProfileStore();
  const { addSource, devices, discoverDevices } = useSourceStore();
  const { ensurePermission } = usePermissionCheck();
  const { FileBrowser, openFilePath: browserOpenFile } = useFileBrowser();

  const [step, setStep] = useState<ModalStep>('select-type');
  const [selectedType, setSelectedType] = useState<SourceType | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [permissionError, setPermissionError] = useState<{ type: SourcePermissionType; message: string } | null>(null);
  const [formData, setFormData] = useState<Source | null>(null);

  // Reset state when modal opens/closes
  // NOTE: We do NOT call discoverDevices() here to avoid triggering camera/screen access
  // before the user selects a source type and permission is checked
  useEffect(() => {
    if (open) {
      setError(null);
      setPermissionError(null);

      // If filterType is provided, skip type selection and go directly to configure
      if (filterType) {
        setSelectedType(filterType);
        setStep('configure');

        // Initialize form data and discover devices for the filtered type
        if (filterType === 'audioDevice') {
          discoverDevices().then(() => {
            // Form data will be set by the auto-select device useEffect
          });
          setFormData(createDefaultAudioDeviceSource('Audio Device', ''));
        }
        // Add other filterType cases here if needed in the future
      } else {
        setStep('select-type');
        setSelectedType(null);
        setFormData(null);
      }
    }
  }, [open, filterType, discoverDevices]);

  // Auto-select first device when devices finish loading (if none selected yet)
  useEffect(() => {
    if (!formData || devices.isDiscovering) return;

    if (formData.type === 'camera' && !formData.deviceId && devices.cameras.length > 0) {
      const first = devices.cameras[0];
      setFormData({ ...formData, deviceId: first.deviceId, name: formData.name || first.name });
    } else if (formData.type === 'screenCapture' && !formData.displayId && devices.displays.length > 0) {
      const first = devices.displays[0];
      setFormData({ ...formData, displayId: first.displayId, deviceName: first.deviceName });
    } else if (formData.type === 'captureCard' && !formData.deviceId && devices.captureCards.length > 0) {
      const first = devices.captureCards[0];
      setFormData({ ...formData, deviceId: first.deviceId, name: formData.name || first.name });
    } else if (formData.type === 'audioDevice' && !formData.deviceId && devices.audioDevices.length > 0) {
      const first = devices.audioDevices[0];
      setFormData({ ...formData, deviceId: first.deviceId, name: formData.name || first.name });
    }
  }, [devices, formData]);

  const handleSelectType = async (type: SourceType) => {
    setPermissionError(null);
    setError(null);

    // Check permissions for device-based sources BEFORE device discovery
    // This ensures macOS permission dialogs appear before camera LED lights up
    if (type === 'camera' || type === 'screenCapture' || type === 'audioDevice') {
      const result = await ensurePermission(type);
      if (!result.granted && result.permission) {
        const permissionLabels: Record<SourcePermissionType, string> = {
          camera: t('permissions.camera', { defaultValue: 'Camera' }),
          microphone: t('permissions.microphone', { defaultValue: 'Microphone' }),
          screenRecording: t('permissions.screenRecording', { defaultValue: 'Screen Recording' }),
        };
        // Use guidance from backend if available, otherwise use generic message
        const message = result.guidance ||
          t('permissions.denied', {
            permission: permissionLabels[result.permission],
            defaultValue: `${permissionLabels[result.permission]} permission is required.`,
          });
        setPermissionError({
          type: result.permission,
          message,
        });
        return;
      }
    }

    // Permission granted or not required - now discover devices
    // This triggers device enumeration AFTER permission dialog, so camera LED
    // won't light up until permission is granted
    if (type === 'camera' || type === 'screenCapture' || type === 'captureCard' || type === 'audioDevice') {
      await discoverDevices();
    }

    setSelectedType(type);
    // Initialize form data with defaults for selected type, auto-selecting first available device
    switch (type) {
      case 'rtmp':
        setFormData(createDefaultRtmpSource());
        break;
      case 'mediaFile':
        setFormData(createDefaultMediaFileSource());
        break;
      case 'screenCapture': {
        const firstDisplay = devices.displays[0];
        setFormData(createDefaultScreenCaptureSource(
          '', // Leave name blank so user must enter one
          firstDisplay?.displayId || '',
          firstDisplay?.deviceName
        ));
        break;
      }
      case 'camera': {
        const firstCamera = devices.cameras[0];
        setFormData(createDefaultCameraSource(
          firstCamera?.name || 'Camera',
          firstCamera?.deviceId || ''
        ));
        break;
      }
      case 'captureCard': {
        const firstCard = devices.captureCards[0];
        setFormData(createDefaultCaptureCardSource(
          firstCard?.name || 'Capture Card',
          firstCard?.deviceId || ''
        ));
        break;
      }
      case 'audioDevice': {
        const firstAudio = devices.audioDevices[0];
        setFormData(createDefaultAudioDeviceSource(
          firstAudio?.name || 'Audio Device',
          firstAudio?.deviceId || ''
        ));
        break;
      }
      case 'color':
        setFormData(createDefaultColorSource());
        break;
      case 'text':
        setFormData(createDefaultTextSource());
        break;
      case 'browser':
        setFormData(createDefaultBrowserSource());
        break;
    }
    setStep('configure');
  };

  const handleBack = () => {
    // If filterType is provided, close the modal instead of going back to type selection
    if (filterType) {
      onClose();
      return;
    }
    setStep('select-type');
    setSelectedType(null);
    setFormData(null);
    setError(null);
    setPermissionError(null);
  };

  const handleSave = async () => {
    if (!formData) return;

    // Validate required fields
    if (!formData.name?.trim()) {
      setError(t('validation.sourceNameRequired', { defaultValue: 'Source name is required' }));
      return;
    }

    // Validate device selection for device-based sources
    if (formData.type === 'camera' && !formData.deviceId) {
      setError(t('validation.cameraDeviceRequired', { defaultValue: 'Please select a camera device' }));
      return;
    }
    if (formData.type === 'captureCard' && !formData.deviceId) {
      setError(t('validation.captureCardRequired', { defaultValue: 'Please select a capture card' }));
      return;
    }
    if (formData.type === 'audioDevice' && !formData.deviceId) {
      setError(t('validation.audioDeviceRequired', { defaultValue: 'Please select an audio device' }));
      return;
    }
    if (formData.type === 'screenCapture' && !formData.displayId) {
      setError(t('validation.displayRequired', { defaultValue: 'Please select a display' }));
      return;
    }
    if (formData.type === 'mediaFile' && !formData.filePath?.trim()) {
      setError(t('validation.filePathRequired', { defaultValue: 'Please select a media file' }));
      return;
    }

    setSaving(true);
    setError(null);

    try {
      // addSource saves to backend and returns updated sources list
      const updatedSources = await addSource(profileName, formData);
      // Update local state with new sources - don't reload profile to avoid overwriting local edits
      setCurrentSources(updatedSources);
      // Notify parent of the newly added source
      onSourceAdded?.(formData);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleBrowseFile = async () => {
    const filters = [
      { name: 'Media Files', extensions: ['mp4', 'mkv', 'avi', 'mov', 'webm', 'mp3', 'wav', 'flac', 'ogg', 'png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'svg', 'html', 'htm'] }
    ];
    const result = backendMode === 'http'
      ? await browserOpenFile({ filters })
      : await dialogs.openFilePath?.({ filters });
    if (result && formData?.type === 'mediaFile') {
      setFormData({ ...formData, filePath: result });
    }
  };

  const renderTypeSelection = () => {
    const availableTypes = SOURCE_TYPES.filter(({ type }) => !excludeTypes.includes(type));
    return (
    <div className="grid grid-cols-2 gap-3">
      {availableTypes.map(({ type, icon }) => (
        <button
          key={type}
          className="flex items-center gap-3 p-5 min-h-[72px] rounded-lg border border-border hover:border-primary hover:bg-primary/5 focus:outline-none focus:ring-2 focus:ring-primary/50 transition-colors text-left"
          onClick={() => handleSelectType(type)}
        >
          <div className="text-primary flex-shrink-0">{icon}</div>
          <div className="min-w-0">
            <div className="font-medium">{getSourceTypeLabel(type)}</div>
            <div className="text-xs text-muted line-clamp-2">
              {t(`stream.sourceTypeDesc.${type}`, { defaultValue: getSourceTypeDescription(type) })}
            </div>
          </div>
        </button>
      ))}
    </div>
    );
  };

  const renderConfigForm = () => {
    if (!formData) return null;

    switch (formData.type) {
      case 'rtmp':
        return renderRtmpForm(formData);
      case 'mediaFile':
        return renderMediaFileForm(formData);
      case 'screenCapture':
        return renderScreenCaptureForm(formData);
      case 'camera':
        return renderCameraForm(formData);
      case 'captureCard':
        return renderCaptureCardForm(formData);
      case 'audioDevice':
        return renderAudioDeviceForm(formData);
      case 'color':
        return renderColorForm(formData);
      case 'text':
        return renderTextForm(formData);
      case 'browser':
        return renderBrowserForm(formData);
      default:
        return null;
    }
  };

  const renderRtmpForm = (data: RtmpSource) => (
    <div className="flex flex-col gap-4">
      <Input
        label={t('stream.sourceName', { defaultValue: 'Source Name' })}
        value={data.name}
        onChange={(e) => setFormData({ ...data, name: e.target.value })}
        placeholder="RTMP Input"
      />
      <Input
        label={t('stream.bindAddress', { defaultValue: 'Bind Address' })}
        value={data.bindAddress}
        onChange={(e) => setFormData({ ...data, bindAddress: e.target.value })}
        helper={t('stream.bindAddressHelper', { defaultValue: 'Use 0.0.0.0 to accept connections from any address' })}
      />
      <div className="grid grid-cols-2 gap-4">
        <Input
          label={t('stream.port', { defaultValue: 'Port' })}
          type="number"
          value={String(data.port)}
          onChange={(e) => setFormData({ ...data, port: parseInt(e.target.value) || 1935 })}
        />
        <Input
          label={t('stream.application', { defaultValue: 'Application' })}
          value={data.application}
          onChange={(e) => setFormData({ ...data, application: e.target.value })}
        />
      </div>
    </div>
  );

  const renderMediaFileForm = (data: MediaFileSource) => (
    <div className="flex flex-col gap-4">
      <Input
        label={t('stream.sourceName', { defaultValue: 'Source Name' })}
        value={data.name}
        onChange={(e) => setFormData({ ...data, name: e.target.value })}
        placeholder="Media File"
      />
      <div className="flex items-end gap-2">
        <div className="flex-1">
          <Input
            label={t('stream.filePath', { defaultValue: 'File Path' })}
            value={data.filePath}
            onChange={(e) => setFormData({ ...data, filePath: e.target.value })}
            placeholder="/path/to/video.mp4"
          />
        </div>
        <Button variant="secondary" className="h-10 px-3" onClick={handleBrowseFile}>
          <FolderOpen className="w-4 h-4" />
        </Button>
      </div>
      <div className="flex items-center justify-between">
        <span className="text-sm">{t('stream.loopPlayback', { defaultValue: 'Loop Playback' })}</span>
        <Toggle
          checked={data.loopPlayback}
          onChange={(checked) => setFormData({ ...data, loopPlayback: checked })}
        />
      </div>
      <div className="flex items-center justify-between">
        <span className="text-sm">{t('stream.audioOnly', { defaultValue: 'Audio Only' })}</span>
        <Toggle
          checked={data.audioOnly ?? false}
          onChange={(checked) => setFormData({ ...data, audioOnly: checked })}
        />
      </div>
    </div>
  );

  const renderScreenCaptureForm = (data: ScreenCaptureSource) => {
    const displayOptions: SelectOption[] = devices.displays.map((d) => ({
      value: d.displayId,
      label: `${d.name} (${d.width}x${d.height})${d.isPrimary ? ' - Primary' : ''}`,
    }));

    return (
      <div className="flex flex-col gap-4">
        <Input
          label={t('stream.sourceName', { defaultValue: 'Source Name' })}
          value={data.name}
          onChange={(e) => setFormData({ ...data, name: e.target.value })}
          placeholder="Screen Capture"
        />
        <div className="flex items-center gap-2">
          <div className="flex-1">
            <Select
              label={t('stream.display', { defaultValue: 'Display' })}
              value={data.displayId}
              onChange={(e) => {
                const selectedDisplay = devices.displays.find(d => d.displayId === e.target.value);
                setFormData({
                  ...data,
                  displayId: e.target.value,
                  deviceName: selectedDisplay?.deviceName,
                  // Auto-fill name only if user hasn't entered one
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
              onClick={() => discoverDevices()}
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
          onChange={(e) => setFormData({ ...data, fps: parseInt(e.target.value) || 30 })}
        />
        <div className="flex items-center justify-between">
          <span className="text-sm">{t('stream.captureCursor', { defaultValue: 'Capture Cursor' })}</span>
          <Toggle
            checked={data.captureCursor}
            onChange={(checked) => setFormData({ ...data, captureCursor: checked })}
          />
        </div>
        <div className="flex items-center justify-between">
          <span className="text-sm">{t('stream.captureAudio', { defaultValue: 'Capture Desktop Audio' })}</span>
          <Toggle
            checked={data.captureAudio}
            onChange={(checked) => setFormData({ ...data, captureAudio: checked })}
          />
        </div>
      </div>
    );
  };

  const renderCameraForm = (data: CameraSource) => {
    const cameraOptions: SelectOption[] = devices.cameras.map((c) => ({
      value: c.deviceId,
      label: c.name,
    }));

    return (
      <div className="flex flex-col gap-4">
        <Input
          label={t('stream.sourceName', { defaultValue: 'Source Name' })}
          value={data.name}
          onChange={(e) => setFormData({ ...data, name: e.target.value })}
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
                setFormData({
                  ...data,
                  deviceId,
                  name: data.name || camera?.name || 'Camera',
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
              onClick={() => discoverDevices()}
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
            onChange={(e) => setFormData({ ...data, width: e.target.value ? parseInt(e.target.value) : undefined })}
            placeholder="1920"
          />
          <Input
            label={t('stream.height', { defaultValue: 'Height' })}
            type="number"
            value={data.height !== undefined ? String(data.height) : ''}
            onChange={(e) => setFormData({ ...data, height: e.target.value ? parseInt(e.target.value) : undefined })}
            placeholder="1080"
          />
          <Input
            label={t('stream.fps', { defaultValue: 'FPS' })}
            type="number"
            value={data.fps !== undefined ? String(data.fps) : ''}
            onChange={(e) => setFormData({ ...data, fps: e.target.value ? parseInt(e.target.value) : undefined })}
            placeholder="30"
          />
        </div>
        <p className="text-xs text-muted">
          {t('stream.cameraResolutionHelper', { defaultValue: 'Leave blank to use device defaults' })}
        </p>
      </div>
    );
  };

  const renderCaptureCardForm = (data: CaptureCardSource) => {
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
          onChange={(e) => setFormData({ ...data, name: e.target.value })}
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
                setFormData({
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
              onClick={() => discoverDevices()}
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
          onChange={(e) => setFormData({ ...data, inputFormat: e.target.value || undefined })}
          options={inputFormatOptions}
        />
      </div>
    );
  };

  const renderAudioDeviceForm = (data: AudioDeviceSource) => {
    const audioDeviceOptions: SelectOption[] = devices.audioDevices.map((d) => ({
      value: d.deviceId,
      label: `${d.name}${d.isDefault ? ' (Default)' : ''}`,
    }));

    return (
      <div className="flex flex-col gap-4">
        <Input
          label={t('stream.sourceName', { defaultValue: 'Source Name' })}
          value={data.name}
          onChange={(e) => setFormData({ ...data, name: e.target.value })}
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
                setFormData({
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
              onClick={() => discoverDevices()}
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
            onChange={(e) => setFormData({ ...data, channels: e.target.value ? parseInt(e.target.value) : undefined })}
            placeholder="2"
          />
          <Input
            label={t('stream.sampleRate', { defaultValue: 'Sample Rate (Hz)' })}
            type="number"
            value={data.sampleRate !== undefined ? String(data.sampleRate) : ''}
            onChange={(e) => setFormData({ ...data, sampleRate: e.target.value ? parseInt(e.target.value) : undefined })}
            placeholder="48000"
          />
        </div>
        <p className="text-xs text-muted">
          {t('stream.audioDeviceHelper', { defaultValue: 'Leave blank to use device defaults' })}
        </p>
      </div>
    );
  };

  const renderColorForm = (data: ColorSource) => {
    const presetColors = ['#000000', '#FFFFFF', '#EF4444', '#22C55E', '#3B82F6', '#7C3AED', '#EC4899', '#F59E0B'];

    return (
      <div className="flex flex-col gap-4">
        <Input
          label={t('stream.sourceName', { defaultValue: 'Source Name' })}
          value={data.name}
          onChange={(e) => setFormData({ ...data, name: e.target.value })}
          placeholder="Color Fill"
        />
        <div className="space-y-2">
          <label className="text-sm font-medium">{t('stream.color', { defaultValue: 'Color' })}</label>
          <div className="flex gap-2">
            <input
              type="color"
              value={data.color}
              onChange={(e) => setFormData({ ...data, color: e.target.value })}
              className="w-12 h-12 rounded cursor-pointer border border-border bg-transparent"
            />
            <Input
              value={data.color}
              onChange={(e) => setFormData({ ...data, color: e.target.value })}
              placeholder="#7C3AED"
              className="flex-1"
            />
          </div>
        </div>
        <div className="space-y-2">
          <label className="text-sm font-medium">{t('stream.presetColors', { defaultValue: 'Preset Colors' })}</label>
          <div className="flex gap-2 flex-wrap">
            {presetColors.map((c) => (
              <button
                key={c}
                type="button"
                onClick={() => setFormData({ ...data, color: c })}
                className={`w-8 h-8 rounded border-2 transition-colors ${
                  data.color.toLowerCase() === c.toLowerCase()
                    ? 'border-primary'
                    : 'border-transparent hover:border-[var(--border-strong)]'
                }`}
                style={{ backgroundColor: c }}
                title={c}
              />
            ))}
          </div>
        </div>
        <div className="space-y-2">
          <label className="text-sm font-medium">
            {t('stream.opacity', { defaultValue: 'Opacity' })}: {Math.round(data.opacity * 100)}%
          </label>
          <input
            type="range"
            min="0"
            max="100"
            value={data.opacity * 100}
            onChange={(e) => setFormData({ ...data, opacity: Number(e.target.value) / 100 })}
            className="w-full h-2 bg-[var(--bg-sunken)] rounded-lg appearance-none cursor-pointer accent-primary"
          />
        </div>
      </div>
    );
  };

  const renderTextForm = (data: TextSource) => {
    const fontOptions: SelectOption[] = [
      { value: 'Arial', label: 'Arial' },
      { value: 'Helvetica', label: 'Helvetica' },
      { value: 'Times New Roman', label: 'Times New Roman' },
      { value: 'Georgia', label: 'Georgia' },
      { value: 'Verdana', label: 'Verdana' },
      { value: 'Courier New', label: 'Courier New' },
      { value: 'Impact', label: 'Impact' },
    ];

    return (
      <div className="flex flex-col gap-4">
        <Input
          label={t('stream.sourceName', { defaultValue: 'Source Name' })}
          value={data.name}
          onChange={(e) => setFormData({ ...data, name: e.target.value })}
          placeholder="Text"
        />
        <div className="space-y-2">
          <label className="text-sm font-medium">{t('stream.textContent', { defaultValue: 'Text Content' })}</label>
          <textarea
            value={data.content}
            onChange={(e) => setFormData({ ...data, content: e.target.value })}
            className="w-full h-24 px-3 py-2 bg-[var(--bg-sunken)] border border-border rounded-lg resize-none focus:outline-none focus:ring-2 focus:ring-primary/50 focus:border-primary"
            placeholder={t('stream.enterText', { defaultValue: 'Enter your text...' })}
          />
        </div>
        <div className="grid grid-cols-2 gap-3">
          <Select
            label={t('stream.font', { defaultValue: 'Font' })}
            value={data.fontFamily}
            onChange={(e) => setFormData({ ...data, fontFamily: e.target.value })}
            options={fontOptions}
          />
          <Input
            label={t('stream.fontSize', { defaultValue: 'Size' })}
            type="number"
            value={String(data.fontSize)}
            onChange={(e) => setFormData({ ...data, fontSize: parseInt(e.target.value) || 48 })}
          />
        </div>
        <div className="flex gap-2">
          <label className="text-sm font-medium mr-2">{t('stream.style', { defaultValue: 'Style' })}</label>
          <button
            type="button"
            onClick={() => setFormData({ ...data, fontWeight: data.fontWeight === 'bold' ? 'normal' : 'bold' })}
            className={`px-4 py-2 rounded font-bold transition-colors ${
              data.fontWeight === 'bold'
                ? 'bg-primary text-primary-foreground'
                : 'bg-[var(--bg-sunken)] hover:bg-[var(--bg-elevated)]'
            }`}
          >
            B
          </button>
          <button
            type="button"
            onClick={() => setFormData({ ...data, fontStyle: data.fontStyle === 'italic' ? 'normal' : 'italic' })}
            className={`px-4 py-2 rounded italic transition-colors ${
              data.fontStyle === 'italic'
                ? 'bg-primary text-primary-foreground'
                : 'bg-[var(--bg-sunken)] hover:bg-[var(--bg-elevated)]'
            }`}
          >
            I
          </button>
        </div>
        <div className="space-y-2">
          <label className="text-sm font-medium">{t('stream.alignment', { defaultValue: 'Alignment' })}</label>
          <div className="flex gap-1 bg-[var(--bg-sunken)] p-1 rounded-lg">
            {(['left', 'center', 'right'] as const).map((align) => (
              <button
                key={align}
                type="button"
                onClick={() => setFormData({ ...data, textAlign: align })}
                className={`flex-1 px-3 py-1.5 text-sm font-medium rounded-md transition-colors ${
                  data.textAlign === align
                    ? 'bg-[var(--bg-base)] text-[var(--text-primary)] shadow-sm'
                    : 'text-[var(--text-muted)] hover:text-[var(--text-secondary)]'
                }`}
              >
                {align.charAt(0).toUpperCase() + align.slice(1)}
              </button>
            ))}
          </div>
        </div>
        <div className="grid grid-cols-2 gap-3">
          <div className="space-y-2">
            <label className="text-sm font-medium">{t('stream.textColor', { defaultValue: 'Text Color' })}</label>
            <input
              type="color"
              value={data.textColor}
              onChange={(e) => setFormData({ ...data, textColor: e.target.value })}
              className="w-full h-10 rounded cursor-pointer border border-border bg-transparent"
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">{t('stream.backgroundColor', { defaultValue: 'Background' })}</label>
            <div className="flex gap-2">
              <input
                type="color"
                value={data.backgroundColor || '#000000'}
                onChange={(e) => setFormData({ ...data, backgroundColor: e.target.value })}
                className="flex-1 h-10 rounded cursor-pointer border border-border bg-transparent"
              />
              <button
                type="button"
                onClick={() => setFormData({ ...data, backgroundColor: undefined })}
                className={`px-3 rounded border transition-colors ${
                  !data.backgroundColor
                    ? 'border-primary text-primary'
                    : 'border-border text-muted hover:border-[var(--border-strong)]'
                }`}
                title={t('stream.noBackground', { defaultValue: 'No background' })}
              >
                ✕
              </button>
            </div>
          </div>
        </div>
      </div>
    );
  };

  const renderBrowserForm = (data: BrowserSource) => {
    const dimensionPresets = [
      { label: '1080p', w: 1920, h: 1080 },
      { label: '720p', w: 1280, h: 720 },
      { label: '480p', w: 854, h: 480 },
    ];

    return (
      <div className="flex flex-col gap-4">
        <Input
          label={t('stream.sourceName', { defaultValue: 'Source Name' })}
          value={data.name}
          onChange={(e) => setFormData({ ...data, name: e.target.value })}
          placeholder="Browser"
        />
        <div className="space-y-2">
          <Input
            label={t('stream.url', { defaultValue: 'URL' })}
            value={data.url}
            onChange={(e) => setFormData({ ...data, url: e.target.value })}
            placeholder="https://example.com"
          />
          <p className="text-xs text-muted">
            {t('stream.browserUrlHelper', { defaultValue: 'Note: Some sites block iframe embedding' })}
          </p>
        </div>
        <div className="space-y-2">
          <label className="text-sm font-medium">{t('stream.dimensions', { defaultValue: 'Dimensions' })}</label>
          <div className="flex gap-2">
            {dimensionPresets.map((preset) => (
              <button
                key={preset.label}
                type="button"
                onClick={() => setFormData({ ...data, width: preset.w, height: preset.h })}
                className={`px-3 py-1.5 rounded text-sm transition-colors ${
                  data.width === preset.w && data.height === preset.h
                    ? 'bg-primary text-primary-foreground'
                    : 'bg-[var(--bg-sunken)] hover:bg-[var(--bg-elevated)]'
                }`}
              >
                {preset.label}
              </button>
            ))}
          </div>
          <div className="grid grid-cols-2 gap-2 mt-2">
            <Input
              label={t('stream.width', { defaultValue: 'Width' })}
              type="number"
              value={String(data.width)}
              onChange={(e) => setFormData({ ...data, width: parseInt(e.target.value) || 1920 })}
            />
            <Input
              label={t('stream.height', { defaultValue: 'Height' })}
              type="number"
              value={String(data.height)}
              onChange={(e) => setFormData({ ...data, height: parseInt(e.target.value) || 1080 })}
            />
          </div>
        </div>
      </div>
    );
  };

  const title = step === 'select-type'
    ? t('stream.addSource', { defaultValue: 'Add Source' })
    : t('stream.configureSource', { defaultValue: `Configure ${getSourceTypeLabel(selectedType!)}` });

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={title}
      footer={
        step === 'configure' ? (
          <>
            <Button variant="ghost" onClick={handleBack} disabled={saving}>
              <ArrowLeft className="w-4 h-4 mr-1" />
              {t('common.back', { defaultValue: 'Back' })}
            </Button>
            <Button onClick={handleSave} disabled={saving}>
              {saving ? t('common.saving', { defaultValue: 'Saving...' }) : t('stream.addSource', { defaultValue: 'Add Source' })}
            </Button>
          </>
        ) : (
          <Button variant="ghost" onClick={onClose}>
            {t('common.cancel', { defaultValue: 'Cancel' })}
          </Button>
        )
      }
    >
      {error && (
        <div className="mb-4 p-3 bg-destructive/20 border border-destructive/30 rounded text-destructive text-sm font-medium">
          {error}
        </div>
      )}
      {permissionError && (
        <div className="mb-4 p-3 bg-[var(--warning-subtle)] border border-[var(--warning-border)] rounded text-[var(--warning-text)] text-sm">
          <p className="font-medium mb-1">
            {t('permissions.required', { defaultValue: 'Permission Required' })}
          </p>
          <p>{permissionError.message}</p>
        </div>
      )}
      {step === 'select-type' ? renderTypeSelection() : renderConfigForm()}

      {/* File browser modal for HTTP mode */}
      <FileBrowser />
    </Modal>
  );
}

function getSourceTypeDescription(type: SourceType): string {
  switch (type) {
    case 'rtmp':
      return 'Receive RTMP stream from encoder';
    case 'mediaFile':
      return 'Play local video or audio file';
    case 'screenCapture':
      return 'Capture display or window';
    case 'camera':
      return 'Webcam or video device';
    case 'captureCard':
      return 'HDMI/SDI capture device';
    case 'audioDevice':
      return 'Microphone or line input';
    case 'color':
      return 'Solid color fill layer';
    case 'text':
      return 'Text overlay with styling';
    case 'browser':
      return 'Web page or widget';
  }
}
