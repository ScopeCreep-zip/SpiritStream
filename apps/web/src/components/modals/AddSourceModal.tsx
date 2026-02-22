/**
 * Add Source Modal
 * Modal for adding new input sources to a profile
 */
import { useState, useEffect, lazy, Suspense } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Radio,
  Film,
  Monitor,
  AppWindow,
  Gamepad2,
  Camera,
  Usb,
  Mic,
  ArrowLeft,
  Palette,
  Type,
  Globe,
  ListVideo,
  Layers,
  Network,
} from 'lucide-react';
import { Modal } from '@/components/ui/Modal';
import { Button } from '@/components/ui/Button';
import { useSourceStore } from '@/stores/sourceStore';
import { usePermissionCheck, type SourcePermissionType } from '@/stores/permissionStore';
import { useSourceActions } from '@/hooks/useSourceActions';
import { dialogs } from '@/lib/backend/dialogs';
import { backendMode } from '@/lib/backend/env';
import { useFileBrowser } from '@/hooks/useFileBrowser';
import type {
  SourceType,
  Source,
} from '@/types/source';
import {
  createDefaultRtmpSource,
  createDefaultMediaFileSource,
  createDefaultScreenCaptureSource,
  createDefaultWindowCaptureSource,
  createDefaultGameCaptureSource,
  createDefaultCameraSource,
  createDefaultCaptureCardSource,
  createDefaultAudioDeviceSource,
  createDefaultColorSource,
  createDefaultTextSource,
  createDefaultBrowserSource,
  createDefaultMediaPlaylistSource,
  createDefaultNestedSceneSource,
  createDefaultNDISource,
  getSourceTypeLabel,
} from '@/types/source';
import { useShallow } from 'zustand/shallow';

// Lazy load form components
const RtmpSourceForm = lazy(() => import('./source-forms/RtmpSourceForm').then(m => ({ default: m.RtmpSourceForm })));
const MediaFileForm = lazy(() => import('./source-forms/MediaFileForm').then(m => ({ default: m.MediaFileForm })));
const ScreenCaptureForm = lazy(() => import('./source-forms/ScreenCaptureForm').then(m => ({ default: m.ScreenCaptureForm })));
const CameraSourceForm = lazy(() => import('./source-forms/CameraSourceForm').then(m => ({ default: m.CameraSourceForm })));
const WindowCaptureForm = lazy(() => import('./source-forms/WindowCaptureForm').then(m => ({ default: m.WindowCaptureForm })));
const GameCaptureForm = lazy(() => import('./source-forms/GameCaptureForm').then(m => ({ default: m.GameCaptureForm })));
const CaptureCardForm = lazy(() => import('./source-forms/CaptureCardForm').then(m => ({ default: m.CaptureCardForm })));
const NdiSourceForm = lazy(() => import('./source-forms/NdiSourceForm').then(m => ({ default: m.NdiSourceForm })));
const AudioDeviceForm = lazy(() => import('./source-forms/AudioDeviceForm').then(m => ({ default: m.AudioDeviceForm })));
const MediaPlaylistForm = lazy(() => import('./source-forms/MediaPlaylistForm').then(m => ({ default: m.MediaPlaylistForm })));
const TextSourceForm = lazy(() => import('./source-forms/TextSourceForm').then(m => ({ default: m.TextSourceForm })));
const BrowserSourceForm = lazy(() => import('./source-forms/BrowserSourceForm').then(m => ({ default: m.BrowserSourceForm })));
const ColorFillForm = lazy(() => import('./source-forms/ColorFillForm').then(m => ({ default: m.ColorFillForm })));
const NestedSceneForm = lazy(() => import('./source-forms/NestedSceneForm').then(m => ({ default: m.NestedSceneForm })));

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
  { type: 'windowCapture', icon: <AppWindow className="w-5 h-5" /> },
  { type: 'gameCapture', icon: <Gamepad2 className="w-5 h-5" /> },
  { type: 'camera', icon: <Camera className="w-5 h-5" /> },
  { type: 'captureCard', icon: <Usb className="w-5 h-5" /> },
  { type: 'audioDevice', icon: <Mic className="w-5 h-5" /> },
  { type: 'color', icon: <Palette className="w-5 h-5" /> },
  { type: 'text', icon: <Type className="w-5 h-5" /> },
  { type: 'browser', icon: <Globe className="w-5 h-5" /> },
  { type: 'mediaPlaylist', icon: <ListVideo className="w-5 h-5" /> },
  { type: 'nestedScene', icon: <Layers className="w-5 h-5" /> },
  { type: 'ndi', icon: <Network className="w-5 h-5" /> },
];

export function AddSourceModal({ open, onClose, profileName, filterType, excludeTypes = [], onSourceAdded }: AddSourceModalProps) {
  const { t } = useTranslation();
  const { devices, discoverDevices, listWindows } = useSourceStore(
    useShallow(s => ({
      devices: s.devices,
      discoverDevices: s.discoverDevices,
      listWindows: s.listWindows
    }))
  );
  const { ensurePermission } = usePermissionCheck();
  const { addSourceToScene } = useSourceActions();
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
      setFormData({
        ...formData,
        deviceId: first.deviceId,
        name: formData.name || first.name,
        linkedAudioDeviceId: first.linkedAudioDeviceId,
      });
    } else if (formData.type === 'screenCapture' && !formData.displayId && devices.displays.length > 0) {
      const first = devices.displays[0];
      setFormData({ ...formData, displayId: first.displayId, deviceName: first.deviceName });
    } else if (formData.type === 'windowCapture' && !formData.windowId && devices.windows.length > 0) {
      const first = devices.windows[0];
      setFormData({ ...formData, windowId: first.windowId, windowTitle: first.title, name: formData.name || first.title });
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
    if (type === 'camera' || type === 'screenCapture' || type === 'windowCapture' || type === 'audioDevice') {
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
    if (type === 'windowCapture') {
      // Parallelize device discovery + window listing for window capture
      await Promise.all([discoverDevices(), listWindows()]);
    } else if (type === 'camera' || type === 'screenCapture' || type === 'captureCard' || type === 'audioDevice') {
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
      case 'windowCapture': {
        const firstWindow = devices.windows[0];
        setFormData(createDefaultWindowCaptureSource(
          firstWindow?.title || 'Window Capture',
          firstWindow?.windowId || '',
          firstWindow?.title || ''
        ));
        break;
      }
      case 'camera': {
        const firstCamera = devices.cameras[0];
        const cameraSource = createDefaultCameraSource(
          firstCamera?.name || 'Camera',
          firstCamera?.deviceId || ''
        );
        // Include linked audio device ID if available
        if (firstCamera?.linkedAudioDeviceId) {
          cameraSource.linkedAudioDeviceId = firstCamera.linkedAudioDeviceId;
        }
        setFormData(cameraSource);
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
      case 'mediaPlaylist':
        setFormData(createDefaultMediaPlaylistSource());
        break;
      case 'nestedScene':
        setFormData(createDefaultNestedSceneSource());
        break;
      case 'gameCapture':
        setFormData(createDefaultGameCaptureSource());
        break;
      case 'ndi':
        setFormData(createDefaultNDISource());
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
      // Consolidated: adds source + linked audio + audio tracks + saves
      await addSourceToScene({ profileName, source: formData });

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

    const commonProps = {
      data: formData,
      onChange: setFormData,
    };

    return (
      <Suspense fallback={<div className="flex items-center justify-center py-8 text-muted">Loading...</div>}>
        {formData.type === 'rtmp' && <RtmpSourceForm {...commonProps} data={formData} />}
        {formData.type === 'mediaFile' && <MediaFileForm {...commonProps} data={formData} onBrowseFile={handleBrowseFile} />}
        {formData.type === 'screenCapture' && <ScreenCaptureForm {...commonProps} data={formData} devices={devices} onRefreshDevices={discoverDevices} />}
        {formData.type === 'camera' && <CameraSourceForm {...commonProps} data={formData} devices={devices} onRefreshDevices={discoverDevices} />}
        {formData.type === 'windowCapture' && <WindowCaptureForm {...commonProps} data={formData} devices={devices} onRefreshWindows={listWindows} />}
        {formData.type === 'gameCapture' && <GameCaptureForm {...commonProps} data={formData} />}
        {formData.type === 'captureCard' && <CaptureCardForm {...commonProps} data={formData} devices={devices} onRefreshDevices={discoverDevices} />}
        {formData.type === 'ndi' && <NdiSourceForm {...commonProps} data={formData} />}
        {formData.type === 'audioDevice' && <AudioDeviceForm {...commonProps} data={formData} devices={devices} onRefreshDevices={discoverDevices} />}
        {formData.type === 'mediaPlaylist' && <MediaPlaylistForm {...commonProps} data={formData} />}
        {formData.type === 'text' && <TextSourceForm {...commonProps} data={formData} />}
        {formData.type === 'browser' && <BrowserSourceForm {...commonProps} data={formData} />}
        {formData.type === 'color' && <ColorFillForm {...commonProps} data={formData} />}
        {formData.type === 'nestedScene' && <NestedSceneForm {...commonProps} data={formData} />}
      </Suspense>
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
      return 'Capture entire display';
    case 'windowCapture':
      return 'Capture specific application window';
    case 'gameCapture':
      return 'Hardware-accelerated game capture';
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
    case 'mediaPlaylist':
      return 'Multiple media files in sequence';
    case 'nestedScene':
      return 'Embed another scene as a source';
    case 'ndi':
      return 'Receive NDI video over network';
  }
}
