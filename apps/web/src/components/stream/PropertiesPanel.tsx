/**
 * Properties Panel
 * Right sidebar showing selected layer properties
 */
import { useState, useEffect, useMemo } from 'react';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
import { Eye, EyeOff, Lock, Unlock, Trash2, Maximize, Move, RefreshCw, RefreshCcw, Monitor, ListVideo } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardBody } from '@/components/ui/Card';
import { Input } from '@/components/ui/Input';
import { Select } from '@/components/ui/Select';
import type { Profile, Scene, SourceLayer, Source, SceneTransition, TransitionType } from '@/types/profile';
import type { ColorSource, TextSource, BrowserSource, MediaPlaylistSource, PlaylistItem, VideoFilter } from '@/types/source';
import { PlaylistEditorModal } from '@/components/modals/PlaylistEditorModal';
import { TRANSITION_TYPES, getTransitionTypeLabel, DEFAULT_TRANSITION } from '@/types/scene';
import { useSceneStore } from '@/stores/sceneStore';
import { useProfileStore } from '@/stores/profileStore';
import { useSourceStore } from '@/stores/sourceStore';
import { api } from '@/lib/backend';
import { toast } from '@/hooks/useToast';
import { VideoFilterSection } from './VideoFilterSection';

interface PropertiesPanelProps {
  profile: Profile;
  scene?: Scene;
  layer?: SourceLayer;
  source?: Source;
}

export function PropertiesPanel({ profile, scene, layer, source }: PropertiesPanelProps) {
  const { t } = useTranslation();
  const { updateLayer, removeLayer, selectLayer } = useSceneStore();
  const { updateCurrentLayer, removeCurrentLayer } = useProfileStore();
  const { devices, discoverDevices, updateSource } = useSourceStore();
  const { updateCurrentSource } = useProfileStore();

  // Local state for editing
  const [x, setX] = useState(0);
  const [y, setY] = useState(0);
  const [width, setWidth] = useState(0);
  const [height, setHeight] = useState(0);
  const [sourceName, setSourceName] = useState('');
  const [isChangingDevice, setIsChangingDevice] = useState(false);

  // Sync local state with layer
  useEffect(() => {
    if (layer) {
      setX(layer.transform.x);
      setY(layer.transform.y);
      setWidth(layer.transform.width);
      setHeight(layer.transform.height);
    }
  }, [layer]);

  // Sync source name with source
  useEffect(() => {
    if (source) {
      setSourceName(source.name);
    }
  }, [source?.name]);

  // Discover devices when source type needs it
  useEffect(() => {
    if (source && ['camera', 'captureCard', 'audioDevice', 'screenCapture'].includes(source.type)) {
      if (!devices.lastDiscovery) {
        discoverDevices();
      }
    }
  }, [source?.type, devices.lastDiscovery, discoverDevices]);

  // Memoize device options based on source type to prevent recalculation on every render
  // IMPORTANT: These hooks must be BEFORE any early returns to satisfy Rules of Hooks
  const deviceOptions = useMemo(() => {
    if (!source) return [];

    switch (source.type) {
      case 'camera':
        return devices.cameras.map((c) => ({ value: c.deviceId, label: c.name }));
      case 'captureCard':
        return devices.captureCards.map((c) => ({ value: c.deviceId, label: c.name }));
      case 'audioDevice':
        return devices.audioDevices.map((d) => ({
          value: d.deviceId,
          label: `${d.name}${d.isDefault ? ' (Default)' : ''}`
        }));
      case 'screenCapture':
        return devices.displays.map((d) => ({ value: d.displayId, label: d.name }));
      default:
        return [];
    }
  }, [source, devices.cameras, devices.captureCards, devices.audioDevices, devices.displays]);

  // Memoize current device ID based on source type
  const currentDeviceId = useMemo(() => {
    if (!source) return '';
    switch (source.type) {
      case 'camera':
      case 'captureCard':
      case 'audioDevice':
        return source.deviceId;
      case 'screenCapture':
        return source.displayId;
      default:
        return '';
    }
  }, [source]);

  const hasDeviceSelector = deviceOptions.length > 0;

  if (!scene) {
    return (
      <Card className="h-full">
        <CardBody className="flex items-center justify-center h-full">
          <p className="text-[var(--text-muted)] text-sm">{t('stream.noSceneSelected', { defaultValue: 'No scene selected' })}</p>
        </CardBody>
      </Card>
    );
  }

  if (!layer) {
    // Show scene properties when no layer is selected
    return (
      <Card className="h-full flex flex-col">
        <CardHeader className="flex-shrink-0" style={{ padding: '12px 16px' }}>
          <CardTitle className="text-sm">{t('stream.sceneProperties', { defaultValue: 'Scene Properties' })}</CardTitle>
        </CardHeader>
        <CardBody className="flex-1 overflow-y-auto" style={{ padding: '12px 16px' }}>
          <ScenePropertiesSection
            profile={profile}
            scene={scene}
            t={t}
          />
        </CardBody>
      </Card>
    );
  }

  const handleUpdateTransform = async () => {
    try {
      const newTransform = {
        ...layer.transform,
        x,
        y,
        width,
        height,
      };
      await updateLayer(profile.name, scene.id, layer.id, { transform: newTransform });
      // Update local state instead of reloading entire profile
      updateCurrentLayer(scene.id, layer.id, { transform: newTransform });
    } catch (err) {
      toast.error(t('stream.updateFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to update: ${err instanceof Error ? err.message : String(err)}` }));
    }
  };

  const handleToggleVisibility = async () => {
    try {
      await updateLayer(profile.name, scene.id, layer.id, { visible: !layer.visible });
      // Update local state instead of reloading entire profile
      updateCurrentLayer(scene.id, layer.id, { visible: !layer.visible });
    } catch (err) {
      toast.error(t('stream.updateFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to update: ${err instanceof Error ? err.message : String(err)}` }));
    }
  };

  const handleToggleLock = async () => {
    try {
      await updateLayer(profile.name, scene.id, layer.id, { locked: !layer.locked });
      // Update local state instead of reloading entire profile
      updateCurrentLayer(scene.id, layer.id, { locked: !layer.locked });
    } catch (err) {
      toast.error(t('stream.updateFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to update: ${err instanceof Error ? err.message : String(err)}` }));
    }
  };

  const handleRemoveLayer = async () => {
    if (confirm(t('stream.confirmRemoveLayer', { defaultValue: 'Remove this layer from the scene?' }))) {
      try {
        await removeLayer(profile.name, scene.id, layer.id);
        selectLayer(null);
        // Update local state instead of reloading entire profile
        removeCurrentLayer(scene.id, layer.id);
      } catch (err) {
        toast.error(t('stream.removeFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to remove: ${err instanceof Error ? err.message : String(err)}` }));
      }
    }
  };

  const handleFillCanvas = async () => {
    const newX = 0;
    const newY = 0;
    const newWidth = scene.canvasWidth;
    const newHeight = scene.canvasHeight;
    setX(newX);
    setY(newY);
    setWidth(newWidth);
    setHeight(newHeight);
    try {
      const newTransform = { ...layer.transform, x: newX, y: newY, width: newWidth, height: newHeight };
      await updateLayer(profile.name, scene.id, layer.id, { transform: newTransform });
      // Update local state instead of reloading entire profile
      updateCurrentLayer(scene.id, layer.id, { transform: newTransform });
    } catch (err) {
      toast.error(t('stream.updateFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to update: ${err instanceof Error ? err.message : String(err)}` }));
    }
  };

  const handleCenter = async () => {
    const newX = Math.round((scene.canvasWidth - width) / 2);
    const newY = Math.round((scene.canvasHeight - height) / 2);
    setX(newX);
    setY(newY);
    try {
      const newTransform = { ...layer.transform, x: newX, y: newY };
      await updateLayer(profile.name, scene.id, layer.id, { transform: newTransform });
      // Update local state instead of reloading entire profile
      updateCurrentLayer(scene.id, layer.id, { transform: newTransform });
    } catch (err) {
      toast.error(t('stream.updateFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to update: ${err instanceof Error ? err.message : String(err)}` }));
    }
  };

  const handleChangeDevice = async (newDeviceId: string) => {
    if (!source || isChangingDevice) return;

    setIsChangingDevice(true);
    try {
      // Stop the current preview so go2rtc releases the old device
      try {
        await api.preview.stopSourcePreview(source.id);
      } catch {
        // Ignore - preview may not be running
      }

      // Build the update based on source type
      const updates: Record<string, string> = source.type === 'screenCapture'
        ? { displayId: newDeviceId }
        : { deviceId: newDeviceId };

      const updatedSource = await updateSource(profile.name, source.id, updates as Partial<Source>);
      // Update local state immediately with the returned source
      updateCurrentSource(updatedSource);
      // The preview will restart automatically with the new device when the component re-renders
    } catch (err) {
      toast.error(t('stream.updateFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to update: ${err instanceof Error ? err.message : String(err)}` }));
    } finally {
      setIsChangingDevice(false);
    }
  };

  const handleUpdateName = async () => {
    if (!source || sourceName === source.name) return;

    try {
      const updatedSource = await updateSource(profile.name, source.id, { name: sourceName } as Partial<Source>);
      updateCurrentSource(updatedSource);
    } catch (err) {
      toast.error(t('stream.updateFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to update: ${err instanceof Error ? err.message : String(err)}` }));
      // Reset to original name on error
      setSourceName(source.name);
    }
  };

  const handleUpdateVideoFilters = async (filters: VideoFilter[]) => {
    try {
      await updateLayer(profile.name, scene.id, layer.id, { videoFilters: filters });
      updateCurrentLayer(scene.id, layer.id, { videoFilters: filters });
    } catch (err) {
      toast.error(t('stream.updateFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to update: ${err instanceof Error ? err.message : String(err)}` }));
    }
  };

  return (
    <Card className="h-full flex flex-col">
      <CardHeader className="flex-shrink-0" style={{ padding: '12px 16px' }}>
        <div className="flex items-center justify-between">
          <CardTitle className="text-sm">{t('stream.properties', { defaultValue: 'Properties' })}</CardTitle>
          <div className="flex items-center gap-0.5">
            <button
              type="button"
              className={`p-1.5 rounded transition-colors ${
                layer.visible
                  ? 'text-primary hover:bg-primary/10'
                  : 'text-[var(--text-muted)] hover:bg-[var(--bg-elevated)]'
              }`}
              onClick={handleToggleVisibility}
              title={layer.visible ? t('common.hide', { defaultValue: 'Hide' }) : t('common.show', { defaultValue: 'Show' })}
            >
              {layer.visible ? <Eye className="w-4 h-4" /> : <EyeOff className="w-4 h-4" />}
            </button>
            <button
              type="button"
              className={`p-1.5 rounded transition-colors ${
                layer.locked
                  ? 'text-[var(--warning)] hover:bg-[var(--warning)]/10'
                  : 'text-[var(--text-muted)] hover:bg-[var(--bg-elevated)]'
              }`}
              onClick={handleToggleLock}
              title={layer.locked ? t('stream.unlock', { defaultValue: 'Unlock' }) : t('stream.lock', { defaultValue: 'Lock' })}
            >
              {layer.locked ? <Lock className="w-4 h-4" /> : <Unlock className="w-4 h-4" />}
            </button>
          </div>
        </div>
      </CardHeader>

      <CardBody className="flex-1 overflow-y-auto" style={{ padding: '12px 16px' }}>
        <div className="space-y-5">
          {/* Lock indicator banner */}
          {layer.locked && (
            <div className="flex items-center gap-2 px-3 py-2 bg-[var(--warning)]/10 border border-[var(--warning)]/20 rounded-md text-[var(--warning)] text-xs">
              <Lock className="w-3.5 h-3.5 flex-shrink-0" />
              <span>{t('stream.layerLocked', { defaultValue: 'Layer is locked. Unlock to edit.' })}</span>
            </div>
          )}

          {/* Source name */}
          <div>
            <h4 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wide mb-3">
              {t('stream.sourceName', { defaultValue: 'Name' })}
            </h4>
            <Input
              type="text"
              value={sourceName}
              onChange={(e) => setSourceName(e.target.value)}
              onBlur={handleUpdateName}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.currentTarget.blur();
                }
              }}
              placeholder={t('stream.sourceNamePlaceholder', { defaultValue: 'Enter source name' })}
            />
          </div>

          {/* Device selector for device-based sources */}
          {hasDeviceSelector && (
            <div>
              <div className="flex items-center justify-between mb-3">
                <h4 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wide">
                  {t('stream.device', { defaultValue: 'Device' })}
                </h4>
                <button
                  type="button"
                  className="p-1 rounded hover:bg-[var(--bg-elevated)] text-[var(--text-muted)] hover:text-[var(--text-secondary)] transition-colors"
                  onClick={() => discoverDevices()}
                  disabled={devices.isDiscovering}
                  title={t('stream.refreshDevices', { defaultValue: 'Refresh devices' })}
                >
                  <RefreshCw className={`w-3.5 h-3.5 ${devices.isDiscovering ? 'animate-spin' : ''}`} />
                </button>
              </div>
              <Select
                value={currentDeviceId}
                onChange={(e) => handleChangeDevice(e.target.value)}
                options={deviceOptions}
                disabled={devices.isDiscovering || isChangingDevice}
              />
            </div>
          )}

          {/* Color source editor */}
          {source?.type === 'color' && (
            <ColorSourceEditor
              source={source as ColorSource}
              profileName={profile.name}
              updateSource={updateSource}
              updateCurrentSource={updateCurrentSource}
              t={t}
            />
          )}

          {/* Text source editor */}
          {source?.type === 'text' && (
            <TextSourceEditor
              source={source as TextSource}
              profileName={profile.name}
              updateSource={updateSource}
              updateCurrentSource={updateCurrentSource}
              t={t}
            />
          )}

          {/* Browser source editor */}
          {source?.type === 'browser' && (
            <BrowserSourceEditor
              source={source as BrowserSource}
              profileName={profile.name}
              updateSource={updateSource}
              updateCurrentSource={updateCurrentSource}
              t={t}
            />
          )}

          {/* Media playlist source editor */}
          {source?.type === 'mediaPlaylist' && (
            <MediaPlaylistSourceEditor
              source={source as MediaPlaylistSource}
              profileName={profile.name}
              updateSource={updateSource}
              updateCurrentSource={updateCurrentSource}
              t={t}
            />
          )}

          {/* Transform section */}
          <div className={layer.locked ? 'opacity-50 pointer-events-none' : ''}>
            <h4 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wide mb-3">
              {t('stream.transform', { defaultValue: 'Transform' })}
            </h4>
            <div className="space-y-3">
              {/* Position */}
              <div className="grid grid-cols-2 gap-2">
                <Input
                  label="X"
                  type="number"
                  value={x}
                  onChange={(e) => setX(Number(e.target.value))}
                  onBlur={handleUpdateTransform}
                  disabled={layer.locked}
                />
                <Input
                  label="Y"
                  type="number"
                  value={y}
                  onChange={(e) => setY(Number(e.target.value))}
                  onBlur={handleUpdateTransform}
                  disabled={layer.locked}
                />
              </div>
              {/* Size */}
              <div className="grid grid-cols-2 gap-2">
                <Input
                  label={t('stream.width', { defaultValue: 'Width' })}
                  type="number"
                  value={width}
                  onChange={(e) => setWidth(Number(e.target.value))}
                  onBlur={handleUpdateTransform}
                  disabled={layer.locked}
                />
                <Input
                  label={t('stream.height', { defaultValue: 'Height' })}
                  type="number"
                  value={height}
                  onChange={(e) => setHeight(Number(e.target.value))}
                  onBlur={handleUpdateTransform}
                  disabled={layer.locked}
                />
              </div>
            </div>
          </div>

          {/* Quick actions */}
          <div className={layer.locked ? 'opacity-50 pointer-events-none' : ''}>
            <h4 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wide mb-3">
              {t('stream.quickActions', { defaultValue: 'Quick Actions' })}
            </h4>
            <div className="grid grid-cols-2 gap-2">
              <button
                type="button"
                className="flex items-center justify-center gap-1.5 px-3 py-2 rounded-md bg-[var(--bg-sunken)] hover:bg-[var(--bg-elevated)] text-[var(--text-secondary)] text-sm transition-colors"
                onClick={handleFillCanvas}
                disabled={layer.locked}
              >
                <Maximize className="w-3.5 h-3.5" />
                {t('stream.fill', { defaultValue: 'Fill' })}
              </button>
              <button
                type="button"
                className="flex items-center justify-center gap-1.5 px-3 py-2 rounded-md bg-[var(--bg-sunken)] hover:bg-[var(--bg-elevated)] text-[var(--text-secondary)] text-sm transition-colors"
                onClick={handleCenter}
                disabled={layer.locked}
              >
                <Move className="w-3.5 h-3.5" />
                {t('stream.center', { defaultValue: 'Center' })}
              </button>
            </div>
          </div>

          {/* Danger zone */}
          <div className="pt-3 border-t border-[var(--border-default)]">
            <button
              type="button"
              className="flex items-center justify-center gap-2 w-full px-3 py-2 rounded-md bg-[var(--error)]/10 hover:bg-[var(--error)]/20 text-[var(--error)] text-sm font-medium transition-colors"
              onClick={handleRemoveLayer}
            >
              <Trash2 className="w-4 h-4" />
              {t('stream.removeFromScene', { defaultValue: 'Remove from Scene' })}
            </button>
          </div>
        </div>

        {/* Video Filters Section */}
        <VideoFilterSection
          layerId={layer.id}
          filters={layer.videoFilters || []}
          onFiltersChange={handleUpdateVideoFilters}
        />
      </CardBody>
    </Card>
  );
}

/**
 * Color Source Editor
 */
interface ColorSourceEditorProps {
  source: ColorSource;
  profileName: string;
  updateSource: (profileName: string, sourceId: string, updates: Partial<Source>) => Promise<Source>;
  updateCurrentSource: (source: Source) => void;
  t: TFunction;
}

function ColorSourceEditor({
  source,
  profileName,
  updateSource,
  updateCurrentSource,
  t,
}: ColorSourceEditorProps) {
  const presetColors = ['#000000', '#FFFFFF', '#EF4444', '#22C55E', '#3B82F6', '#7C3AED', '#EC4899', '#F59E0B'];

  const handleUpdate = async (updates: Partial<ColorSource>) => {
    try {
      const updated = await updateSource(profileName, source.id, updates as Partial<Source>);
      updateCurrentSource(updated);
    } catch (err) {
      toast.error(`Failed to update: ${err instanceof Error ? err.message : String(err)}`);
    }
  };

  return (
    <div className="space-y-4">
      <h4 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wide">
        {t('stream.colorSettings', { defaultValue: 'Color Settings' })}
      </h4>
      <div className="space-y-3">
        <div className="flex gap-2">
          <input
            type="color"
            value={source.color}
            onChange={(e) => handleUpdate({ color: e.target.value })}
            className="w-12 h-10 rounded cursor-pointer border border-[var(--border-default)] bg-transparent"
          />
          <Input
            value={source.color}
            onChange={(e) => handleUpdate({ color: e.target.value })}
            placeholder="#7C3AED"
            className="flex-1"
          />
        </div>
        <div className="flex gap-1.5 flex-wrap">
          {presetColors.map((c) => (
            <button
              key={c}
              type="button"
              onClick={() => handleUpdate({ color: c })}
              className={`w-6 h-6 rounded border-2 transition-colors ${
                source.color.toLowerCase() === c.toLowerCase()
                  ? 'border-primary'
                  : 'border-transparent hover:border-[var(--border-strong)]'
              }`}
              style={{ backgroundColor: c }}
              title={c}
            />
          ))}
        </div>
        <div className="space-y-1">
          <label className="text-xs text-[var(--text-muted)]">
            {t('stream.opacity', { defaultValue: 'Opacity' })}: {Math.round(source.opacity * 100)}%
          </label>
          <input
            type="range"
            min="0"
            max="100"
            value={source.opacity * 100}
            onChange={(e) => handleUpdate({ opacity: Number(e.target.value) / 100 })}
            className="w-full h-2 bg-[var(--bg-sunken)] rounded-lg appearance-none cursor-pointer accent-primary"
          />
        </div>
      </div>
    </div>
  );
}

/**
 * Text Source Editor
 */
interface TextSourceEditorProps {
  source: TextSource;
  profileName: string;
  updateSource: (profileName: string, sourceId: string, updates: Partial<Source>) => Promise<Source>;
  updateCurrentSource: (source: Source) => void;
  t: TFunction;
}

function TextSourceEditor({
  source,
  profileName,
  updateSource,
  updateCurrentSource,
  t,
}: TextSourceEditorProps) {
  const handleUpdate = async (updates: Partial<TextSource>) => {
    try {
      const updated = await updateSource(profileName, source.id, updates as Partial<Source>);
      updateCurrentSource(updated);
    } catch (err) {
      toast.error(`Failed to update: ${err instanceof Error ? err.message : String(err)}`);
    }
  };

  return (
    <div className="space-y-4">
      <h4 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wide">
        {t('stream.textSettings', { defaultValue: 'Text Settings' })}
      </h4>
      <div className="space-y-3">
        {/* Text content */}
        <div>
          <label className="text-xs text-[var(--text-muted)] mb-1 block">
            {t('stream.content', { defaultValue: 'Content' })}
          </label>
          <textarea
            value={source.content}
            onChange={(e) => handleUpdate({ content: e.target.value })}
            className="w-full h-20 px-3 py-2 bg-[var(--bg-sunken)] border border-[var(--border-default)] rounded-lg resize-none text-sm focus:outline-none focus:ring-2 focus:ring-primary/50"
            placeholder={t('stream.enterText', { defaultValue: 'Enter text...' })}
          />
        </div>

        {/* Font controls */}
        <div className="grid grid-cols-2 gap-2">
          <Select
            label={t('stream.font', { defaultValue: 'Font' })}
            value={source.fontFamily}
            onChange={(e) => handleUpdate({ fontFamily: e.target.value })}
            options={[
              { value: 'Arial', label: 'Arial' },
              { value: 'Helvetica', label: 'Helvetica' },
              { value: 'Times New Roman', label: 'Times' },
              { value: 'Georgia', label: 'Georgia' },
              { value: 'Verdana', label: 'Verdana' },
              { value: 'Impact', label: 'Impact' },
            ]}
          />
          <Input
            label={t('stream.size', { defaultValue: 'Size' })}
            type="number"
            value={source.fontSize}
            onChange={(e) => handleUpdate({ fontSize: parseInt(e.target.value) || 48 })}
          />
        </div>

        {/* Style toggles */}
        <div className="flex gap-1">
          <button
            type="button"
            onClick={() => handleUpdate({ fontWeight: source.fontWeight === 'bold' ? 'normal' : 'bold' })}
            className={`px-3 py-1.5 rounded text-sm font-bold transition-colors ${
              source.fontWeight === 'bold'
                ? 'bg-primary text-primary-foreground'
                : 'bg-[var(--bg-sunken)] hover:bg-[var(--bg-elevated)]'
            }`}
          >
            B
          </button>
          <button
            type="button"
            onClick={() => handleUpdate({ fontStyle: source.fontStyle === 'italic' ? 'normal' : 'italic' })}
            className={`px-3 py-1.5 rounded text-sm italic transition-colors ${
              source.fontStyle === 'italic'
                ? 'bg-primary text-primary-foreground'
                : 'bg-[var(--bg-sunken)] hover:bg-[var(--bg-elevated)]'
            }`}
          >
            I
          </button>
          <div className="flex-1" />
          {(['left', 'center', 'right'] as const).map((align) => (
            <button
              key={align}
              type="button"
              onClick={() => handleUpdate({ textAlign: align })}
              className={`px-3 py-1.5 rounded text-xs transition-colors ${
                source.textAlign === align
                  ? 'bg-primary text-primary-foreground'
                  : 'bg-[var(--bg-sunken)] hover:bg-[var(--bg-elevated)]'
              }`}
            >
              {align.charAt(0).toUpperCase()}
            </button>
          ))}
        </div>

        {/* Colors */}
        <div className="grid grid-cols-2 gap-2">
          <div>
            <label className="text-xs text-[var(--text-muted)] mb-1 block">
              {t('stream.textColor', { defaultValue: 'Text' })}
            </label>
            <input
              type="color"
              value={source.textColor}
              onChange={(e) => handleUpdate({ textColor: e.target.value })}
              className="w-full h-8 rounded cursor-pointer border border-[var(--border-default)] bg-transparent"
            />
          </div>
          <div>
            <label className="text-xs text-[var(--text-muted)] mb-1 block">
              {t('stream.background', { defaultValue: 'Background' })}
            </label>
            <div className="flex gap-1">
              <input
                type="color"
                value={source.backgroundColor || '#000000'}
                onChange={(e) => handleUpdate({ backgroundColor: e.target.value })}
                className="flex-1 h-8 rounded cursor-pointer border border-[var(--border-default)] bg-transparent"
              />
              <button
                type="button"
                onClick={() => handleUpdate({ backgroundColor: undefined })}
                className={`px-2 h-8 rounded border text-xs transition-colors ${
                  !source.backgroundColor
                    ? 'border-primary text-primary'
                    : 'border-[var(--border-default)] text-[var(--text-muted)] hover:border-[var(--border-strong)]'
                }`}
                title={t('stream.noBackground', { defaultValue: 'No background' })}
              >
                ✕
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

/**
 * Browser Source Editor
 */
interface BrowserSourceEditorProps {
  source: BrowserSource;
  profileName: string;
  updateSource: (profileName: string, sourceId: string, updates: Partial<Source>) => Promise<Source>;
  updateCurrentSource: (source: Source) => void;
  t: TFunction;
}

function BrowserSourceEditor({
  source,
  profileName,
  updateSource,
  updateCurrentSource,
  t,
}: BrowserSourceEditorProps) {
  // Local state for editing - prevents API calls on every keystroke
  const [url, setUrl] = useState(source.url);
  const [width, setWidth] = useState(source.width);
  const [height, setHeight] = useState(source.height);

  // Sync local state with source when it changes externally
  useEffect(() => {
    setUrl(source.url);
    setWidth(source.width);
    setHeight(source.height);
  }, [source.url, source.width, source.height]);

  const handleUpdate = async (updates: Partial<BrowserSource>) => {
    try {
      const updated = await updateSource(profileName, source.id, updates as Partial<Source>);
      updateCurrentSource(updated);
    } catch (err) {
      toast.error(`Failed to update: ${err instanceof Error ? err.message : String(err)}`);
      // Reset local state on error
      setUrl(source.url);
      setWidth(source.width);
      setHeight(source.height);
    }
  };

  const handleRefresh = () => {
    // Trigger a refresh by updating the refreshToken
    handleUpdate({ refreshToken: crypto.randomUUID() });
  };

  return (
    <div className="space-y-4">
      <h4 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wide">
        {t('stream.browserSettings', { defaultValue: 'Browser Settings' })}
      </h4>
      <div className="space-y-3">
        <Input
          label={t('stream.url', { defaultValue: 'URL' })}
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          onBlur={() => {
            if (url !== source.url) {
              handleUpdate({ url });
            }
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.currentTarget.blur();
            }
          }}
          placeholder="https://example.com"
        />
        <div className="grid grid-cols-2 gap-2">
          <Input
            label={t('stream.width', { defaultValue: 'Width' })}
            type="number"
            value={width}
            onChange={(e) => setWidth(parseInt(e.target.value) || 1920)}
            onBlur={() => {
              if (width !== source.width) {
                handleUpdate({ width });
              }
            }}
          />
          <Input
            label={t('stream.height', { defaultValue: 'Height' })}
            type="number"
            value={height}
            onChange={(e) => setHeight(parseInt(e.target.value) || 1080)}
            onBlur={() => {
              if (height !== source.height) {
                handleUpdate({ height });
              }
            }}
          />
        </div>
        <button
          type="button"
          onClick={handleRefresh}
          className="flex items-center justify-center gap-2 w-full px-3 py-2 bg-[var(--bg-sunken)] hover:bg-[var(--bg-elevated)] rounded-lg text-sm transition-colors"
        >
          <RefreshCcw className="w-4 h-4" />
          {t('stream.refreshPage', { defaultValue: 'Refresh Page' })}
        </button>
      </div>
    </div>
  );
}

/**
 * Media Playlist Source Editor
 */
interface MediaPlaylistSourceEditorProps {
  source: MediaPlaylistSource;
  profileName: string;
  updateSource: (profileName: string, sourceId: string, updates: Partial<Source>) => Promise<Source>;
  updateCurrentSource: (source: Source) => void;
  t: TFunction;
}

function MediaPlaylistSourceEditor({
  source,
  profileName,
  updateSource,
  updateCurrentSource,
  t,
}: MediaPlaylistSourceEditorProps) {
  const [isEditorOpen, setIsEditorOpen] = useState(false);

  const handleSavePlaylist = async (items: PlaylistItem[]) => {
    try {
      const updated = await updateSource(profileName, source.id, { items } as Partial<Source>);
      updateCurrentSource(updated);
      setIsEditorOpen(false);
    } catch (err) {
      console.error('[MediaPlaylistSourceEditor] Failed to save:', err);
    }
  };

  const handleUpdateSetting = async (updates: Partial<MediaPlaylistSource>) => {
    try {
      const updated = await updateSource(profileName, source.id, updates as Partial<Source>);
      updateCurrentSource(updated);
    } catch (err) {
      console.error('[MediaPlaylistSourceEditor] Failed to update:', err);
    }
  };

  return (
    <div className="space-y-4">
      <h4 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wide">
        {t('stream.playlistSettings', { defaultValue: 'Playlist Settings' })}
      </h4>

      {/* Playlist items summary and edit button */}
      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <span className="text-sm text-[var(--text-secondary)]">
            {t('stream.playlistItems', { count: source.items.length, defaultValue: `${source.items.length} items` })}
          </span>
          <button
            type="button"
            onClick={() => setIsEditorOpen(true)}
            className="flex items-center gap-2 px-3 py-1.5 bg-[var(--bg-sunken)] hover:bg-[var(--bg-elevated)] rounded-lg text-sm transition-colors"
          >
            <ListVideo className="w-4 h-4" />
            {t('stream.editPlaylist', { defaultValue: 'Edit Playlist' })}
          </button>
        </div>

        {/* Current item preview */}
        {source.items.length > 0 && (
          <div className="p-2 bg-[var(--bg-sunken)] rounded text-xs">
            <span className="text-[var(--text-muted)]">{t('stream.currentItem', { defaultValue: 'Now Playing:' })}</span>
            <span className="ml-2 text-[var(--text-primary)]">
              {source.items[source.currentItemIndex]?.name || 'Unknown'}
            </span>
          </div>
        )}
      </div>

      {/* Playback settings */}
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <span className="text-sm">{t('stream.autoAdvance', { defaultValue: 'Auto Advance' })}</span>
          <button
            type="button"
            onClick={() => handleUpdateSetting({ autoAdvance: !source.autoAdvance })}
            className={`w-10 h-5 rounded-full transition-colors ${
              source.autoAdvance ? 'bg-primary' : 'bg-[var(--bg-sunken)]'
            }`}
          >
            <div
              className={`w-4 h-4 rounded-full bg-white shadow transition-transform ${
                source.autoAdvance ? 'translate-x-5' : 'translate-x-0.5'
              }`}
            />
          </button>
        </div>

        <div className="flex items-center justify-between">
          <span className="text-sm">{t('stream.fadeBetweenItems', { defaultValue: 'Fade Between Items' })}</span>
          <button
            type="button"
            onClick={() => handleUpdateSetting({ fadeBetweenItems: !source.fadeBetweenItems })}
            className={`w-10 h-5 rounded-full transition-colors ${
              source.fadeBetweenItems ? 'bg-primary' : 'bg-[var(--bg-sunken)]'
            }`}
          >
            <div
              className={`w-4 h-4 rounded-full bg-white shadow transition-transform ${
                source.fadeBetweenItems ? 'translate-x-5' : 'translate-x-0.5'
              }`}
            />
          </button>
        </div>

        <Select
          label={t('stream.shuffleMode', { defaultValue: 'Shuffle Mode' })}
          value={source.shuffleMode}
          onChange={(e) => handleUpdateSetting({ shuffleMode: e.target.value as 'none' | 'all' | 'repeat-one' })}
          options={[
            { value: 'none', label: t('stream.shuffleNone', { defaultValue: 'None' }) },
            { value: 'all', label: t('stream.shuffleAll', { defaultValue: 'Shuffle All' }) },
            { value: 'repeat-one', label: t('stream.repeatOne', { defaultValue: 'Repeat One' }) },
          ]}
        />
      </div>

      {/* Playlist editor modal */}
      <PlaylistEditorModal
        open={isEditorOpen}
        onClose={() => setIsEditorOpen(false)}
        source={source}
        onSave={handleSavePlaylist}
      />
    </div>
  );
}

/**
 * Scene Properties Section
 * Shown when no layer is selected - allows editing scene name and transition settings
 */
interface ScenePropertiesSectionProps {
  profile: Profile;
  scene: Scene;
  t: TFunction;
}

function ScenePropertiesSection({ profile, scene, t }: ScenePropertiesSectionProps) {
  const { updateCurrentScene, updateCurrentProfile } = useProfileStore();
  const { updateScene } = useSceneStore();

  const [sceneName, setSceneName] = useState(scene.name);

  // Sync local state with scene
  useEffect(() => {
    setSceneName(scene.name);
  }, [scene.name]);

  // Get effective transition (scene override or profile default)
  const effectiveTransition = scene.transitionIn || profile.defaultTransition || DEFAULT_TRANSITION;
  const hasOverride = !!scene.transitionIn;

  const handleUpdateSceneName = async () => {
    if (sceneName === scene.name) return;
    try {
      await updateScene(profile.name, scene.id, { name: sceneName });
      updateCurrentScene(scene.id, { name: sceneName });
    } catch (err) {
      toast.error(`Failed to update: ${err instanceof Error ? err.message : String(err)}`);
      setSceneName(scene.name);
    }
  };

  const handleUpdateSceneTransition = async (updates: Partial<SceneTransition>) => {
    try {
      const newTransition: SceneTransition = {
        ...effectiveTransition,
        ...updates,
      };
      await updateScene(profile.name, scene.id, { transitionIn: newTransition });
      updateCurrentScene(scene.id, { transitionIn: newTransition });
    } catch (err) {
      toast.error(`Failed to update: ${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const handleUseDefaultTransition = async () => {
    try {
      await updateScene(profile.name, scene.id, { transitionIn: undefined });
      updateCurrentScene(scene.id, { transitionIn: undefined });
    } catch (err) {
      toast.error(`Failed to update: ${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const handleUpdateProfileDefaultTransition = async (updates: Partial<SceneTransition>) => {
    try {
      const newTransition: SceneTransition = {
        ...(profile.defaultTransition || DEFAULT_TRANSITION),
        ...updates,
      };
      // Update profile default transition
      await api.profile.save({ ...profile, defaultTransition: newTransition });
      updateCurrentProfile({ defaultTransition: newTransition });
    } catch (err) {
      toast.error(`Failed to update: ${err instanceof Error ? err.message : String(err)}`);
    }
  };

  return (
    <div className="space-y-5">
      {/* Scene name */}
      <div>
        <h4 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wide mb-3">
          {t('stream.sceneName', { defaultValue: 'Name' })}
        </h4>
        <Input
          type="text"
          value={sceneName}
          onChange={(e) => setSceneName(e.target.value)}
          onBlur={handleUpdateSceneName}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.currentTarget.blur();
            }
          }}
          placeholder={t('stream.sceneNamePlaceholder', { defaultValue: 'Scene name' })}
        />
      </div>

      {/* Canvas dimensions (read-only info) */}
      <div>
        <h4 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wide mb-3">
          {t('stream.canvas', { defaultValue: 'Canvas' })}
        </h4>
        <div className="flex items-center gap-2 px-3 py-2 bg-[var(--bg-sunken)] rounded-lg">
          <Monitor className="w-4 h-4 text-[var(--text-muted)]" />
          <span className="text-sm text-[var(--text-secondary)]">
            {scene.canvasWidth} × {scene.canvasHeight}
          </span>
        </div>
      </div>

      {/* Transition settings */}
      <div className="border-t border-[var(--border-default)] pt-4">
        <div className="flex items-center justify-between mb-3">
          <h4 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wide">
            {t('stream.transitionIn', { defaultValue: 'Transition In' })}
          </h4>
          {hasOverride && (
            <button
              type="button"
              onClick={handleUseDefaultTransition}
              className="text-xs text-primary hover:text-primary/80 transition-colors"
            >
              {t('stream.useDefault', { defaultValue: 'Use Default' })}
            </button>
          )}
        </div>

        <div className="space-y-3">
          {/* Transition type selector */}
          <Select
            label={t('stream.transitionType', { defaultValue: 'Type' })}
            value={effectiveTransition.type}
            onChange={(e) => {
              const newType = e.target.value as TransitionType;
              if (hasOverride || !profile.defaultTransition) {
                handleUpdateSceneTransition({ type: newType });
              } else {
                // If using default, create an override
                handleUpdateSceneTransition({ type: newType, durationMs: effectiveTransition.durationMs });
              }
            }}
            options={TRANSITION_TYPES.map((type) => ({
              value: type,
              label: getTransitionTypeLabel(type),
            }))}
          />

          {/* Duration slider (hidden for 'cut' which is instant) */}
          {effectiveTransition.type !== 'cut' && (
            <div className="space-y-1">
              <label className="text-xs text-[var(--text-muted)]">
                {t('stream.duration', { defaultValue: 'Duration' })}: {effectiveTransition.durationMs}ms
              </label>
              <input
                type="range"
                min="100"
                max="2000"
                step="50"
                value={effectiveTransition.durationMs}
                onChange={(e) => {
                  const durationMs = Number(e.target.value);
                  if (hasOverride || !profile.defaultTransition) {
                    handleUpdateSceneTransition({ durationMs });
                  } else {
                    handleUpdateSceneTransition({ type: effectiveTransition.type, durationMs });
                  }
                }}
                className="w-full h-2 bg-[var(--bg-sunken)] rounded-lg appearance-none cursor-pointer accent-primary"
              />
              <div className="flex justify-between text-[10px] text-[var(--text-muted)]">
                <span>100ms</span>
                <span>2000ms</span>
              </div>
            </div>
          )}

          {/* Override indicator */}
          {hasOverride && (
            <div className="flex items-center gap-2 px-2 py-1.5 bg-primary/10 border border-primary/20 rounded text-xs text-primary">
              <span>{t('stream.customTransition', { defaultValue: 'Custom transition for this scene' })}</span>
            </div>
          )}
        </div>
      </div>

      {/* Profile default transition section */}
      <div className="border-t border-[var(--border-default)] pt-4">
        <h4 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wide mb-3">
          {t('stream.defaultTransition', { defaultValue: 'Profile Default' })}
        </h4>
        <div className="space-y-3">
          <Select
            label={t('stream.transitionType', { defaultValue: 'Type' })}
            value={(profile.defaultTransition || DEFAULT_TRANSITION).type}
            onChange={(e) => handleUpdateProfileDefaultTransition({ type: e.target.value as TransitionType })}
            options={TRANSITION_TYPES.map((type) => ({
              value: type,
              label: getTransitionTypeLabel(type),
            }))}
          />
          {(profile.defaultTransition || DEFAULT_TRANSITION).type !== 'cut' && (
            <div className="space-y-1">
              <label className="text-xs text-[var(--text-muted)]">
                {t('stream.duration', { defaultValue: 'Duration' })}: {(profile.defaultTransition || DEFAULT_TRANSITION).durationMs}ms
              </label>
              <input
                type="range"
                min="100"
                max="2000"
                step="50"
                value={(profile.defaultTransition || DEFAULT_TRANSITION).durationMs}
                onChange={(e) => handleUpdateProfileDefaultTransition({ durationMs: Number(e.target.value) })}
                className="w-full h-2 bg-[var(--bg-sunken)] rounded-lg appearance-none cursor-pointer accent-primary"
              />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
