/**
 * Properties Panel
 * Right sidebar showing selected layer properties
 */
import React, { useState, useEffect, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Eye, EyeOff, Lock, Unlock, Trash2, Maximize, Move, RefreshCw } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardBody } from '@/components/ui/Card';
import { Input } from '@/components/ui/Input';
import { Select } from '@/components/ui/Select';
import type { Profile, Scene, SourceLayer, Source } from '@/types/profile';
import type { ColorSource, TextSource, BrowserSource, MediaPlaylistSource, VideoFilter } from '@/types/source';
import { useSceneStore } from '@/stores/sceneStore';
import { useProfileStore } from '@/stores/profileStore';
import { useSourceStore } from '@/stores/sourceStore';
import { api } from '@/lib/backend';
import { createErrorHandler } from '@/hooks/useToast';
import { VideoFilterSection } from './VideoFilterSection';
import { blurOnEnter } from '@/utils/inputHandlers';
import {
  ColorSourceEditor,
  TextSourceEditor,
  BrowserSourceEditor,
  MediaPlaylistSourceEditor,
  ScenePropertiesSection,
} from './properties';
import { useShallow } from 'zustand/shallow';

interface PropertiesPanelProps {
  profile: Profile;
  scene?: Scene;
  layer?: SourceLayer;
  source?: Source;
}

export const PropertiesPanel = React.memo(function PropertiesPanel({ profile, scene, layer, source }: PropertiesPanelProps) {
  const { t } = useTranslation();
  const { updateLayer, removeLayer, selectLayer } = useSceneStore(
    useShallow(s => ({
      updateLayer: s.updateLayer,
      removeLayer: s.removeLayer,
      selectLayer: s.selectLayer
    }))
  );
  const { updateCurrentLayer, removeCurrentLayer, updateCurrentSource, updateSource } = useProfileStore(
    useShallow(s => ({
      updateCurrentLayer: s.updateCurrentLayer,
      removeCurrentLayer: s.removeCurrentLayer,
      updateCurrentSource: s.updateCurrentSource,
      updateSource: s.updateSource,
    }))
  );
  const { devices, discoverDevices } = useSourceStore(
    useShallow(s => ({
      devices: s.devices,
      discoverDevices: s.discoverDevices,
    }))
  );

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

  // Memoize device options based on source type
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

  // Create reusable error handlers
  const handleUpdateError = createErrorHandler(t, 'stream.updateFailed', 'Failed to update');
  const handleRemoveError = createErrorHandler(t, 'stream.removeFailed', 'Failed to remove');

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
    return (
      <Card className="h-full flex flex-col">
        <CardHeader className="flex-shrink-0 py-3 px-4">
          <CardTitle className="text-sm">{t('stream.sceneProperties', { defaultValue: 'Scene Properties' })}</CardTitle>
        </CardHeader>
        <CardBody className="flex-1 overflow-y-auto py-3 px-4">
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
      const newTransform = { ...layer.transform, x, y, width, height };
      await updateLayer(profile.name, scene.id, layer.id, { transform: newTransform });
      updateCurrentLayer(scene.id, layer.id, { transform: newTransform });
    } catch (err) {
      handleUpdateError(err);
    }
  };

  const handleToggleVisibility = async () => {
    try {
      await updateLayer(profile.name, scene.id, layer.id, { visible: !layer.visible });
      updateCurrentLayer(scene.id, layer.id, { visible: !layer.visible });
    } catch (err) {
      handleUpdateError(err);
    }
  };

  const handleToggleLock = async () => {
    try {
      await updateLayer(profile.name, scene.id, layer.id, { locked: !layer.locked });
      updateCurrentLayer(scene.id, layer.id, { locked: !layer.locked });
    } catch (err) {
      handleUpdateError(err);
    }
  };

  const handleRemoveLayer = async () => {
    if (confirm(t('stream.confirmRemoveLayer', { defaultValue: 'Remove this layer from the scene?' }))) {
      try {
        await removeLayer(profile.name, scene.id, layer.id);
        selectLayer(null);
        removeCurrentLayer(scene.id, layer.id);
      } catch (err) {
        handleRemoveError(err);
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
      updateCurrentLayer(scene.id, layer.id, { transform: newTransform });
    } catch (err) {
      handleUpdateError(err);
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
      updateCurrentLayer(scene.id, layer.id, { transform: newTransform });
    } catch (err) {
      handleUpdateError(err);
    }
  };

  const handleChangeDevice = async (newDeviceId: string) => {
    if (!source || isChangingDevice) return;

    setIsChangingDevice(true);
    try {
      try {
        await api.preview.stopSourcePreview(source.id);
      } catch {
        // Ignore - preview may not be running
      }

      const updates: Record<string, string> = source.type === 'screenCapture'
        ? { displayId: newDeviceId }
        : { deviceId: newDeviceId };

      const updatedSource = await updateSource(profile.name, source.id, updates as Partial<Source>);
      updateCurrentSource(updatedSource);
    } catch (err) {
      handleUpdateError(err);
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
      handleUpdateError(err);
      setSourceName(source.name);
    }
  };

  const handleUpdateVideoFilters = async (filters: VideoFilter[]) => {
    try {
      await updateLayer(profile.name, scene.id, layer.id, { videoFilters: filters });
      updateCurrentLayer(scene.id, layer.id, { videoFilters: filters });
    } catch (err) {
      handleUpdateError(err);
    }
  };

  return (
    <Card className="h-full flex flex-col">
      <CardHeader className="flex-shrink-0 py-3 px-4">
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

      <CardBody className="flex-1 overflow-y-auto py-3 px-4">
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
              onKeyDown={blurOnEnter}
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

          {/* Source type editors */}
          {source?.type === 'color' && (
            <ColorSourceEditor
              source={source as ColorSource}
              profileName={profile.name}
              updateSource={updateSource}
              updateCurrentSource={updateCurrentSource}
              t={t}
            />
          )}

          {source?.type === 'text' && (
            <TextSourceEditor
              source={source as TextSource}
              profileName={profile.name}
              updateSource={updateSource}
              updateCurrentSource={updateCurrentSource}
              t={t}
            />
          )}

          {source?.type === 'browser' && (
            <BrowserSourceEditor
              source={source as BrowserSource}
              profileName={profile.name}
              updateSource={updateSource}
              updateCurrentSource={updateCurrentSource}
              t={t}
            />
          )}

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
              <div className="grid grid-cols-2 gap-2">
                <Input label="X" type="number" value={x} onChange={(e) => setX(Number(e.target.value))} onBlur={handleUpdateTransform} disabled={layer.locked} />
                <Input label="Y" type="number" value={y} onChange={(e) => setY(Number(e.target.value))} onBlur={handleUpdateTransform} disabled={layer.locked} />
              </div>
              <div className="grid grid-cols-2 gap-2">
                <Input label={t('stream.width', { defaultValue: 'Width' })} type="number" value={width} onChange={(e) => setWidth(Number(e.target.value))} onBlur={handleUpdateTransform} disabled={layer.locked} />
                <Input label={t('stream.height', { defaultValue: 'Height' })} type="number" value={height} onChange={(e) => setHeight(Number(e.target.value))} onBlur={handleUpdateTransform} disabled={layer.locked} />
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
});
