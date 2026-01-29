/**
 * Properties Panel
 * Right sidebar showing selected layer properties
 */
import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Eye, EyeOff, Lock, Unlock, Trash2, Maximize, Move } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardBody } from '@/components/ui/Card';
import { Input } from '@/components/ui/Input';
import type { Profile, Scene, SourceLayer, Source } from '@/types/profile';
import { useSceneStore } from '@/stores/sceneStore';
import { useProfileStore } from '@/stores/profileStore';
import { toast } from '@/hooks/useToast';

interface PropertiesPanelProps {
  profile: Profile;
  scene?: Scene;
  layer?: SourceLayer;
  source?: Source;
}

export function PropertiesPanel({ profile, scene, layer, source }: PropertiesPanelProps) {
  const { t } = useTranslation();
  const { updateLayer, removeLayer, selectLayer } = useSceneStore();
  const { reloadProfile } = useProfileStore();

  // Local state for editing
  const [x, setX] = useState(0);
  const [y, setY] = useState(0);
  const [width, setWidth] = useState(0);
  const [height, setHeight] = useState(0);

  // Sync local state with layer
  useEffect(() => {
    if (layer) {
      setX(layer.transform.x);
      setY(layer.transform.y);
      setWidth(layer.transform.width);
      setHeight(layer.transform.height);
    }
  }, [layer]);

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
        <CardHeader className="flex-shrink-0" style={{ padding: '12px 16px' }}>
          <CardTitle className="text-sm">{t('stream.properties', { defaultValue: 'Properties' })}</CardTitle>
        </CardHeader>
        <CardBody className="flex-1 flex items-center justify-center" style={{ padding: '12px 16px' }}>
          <p className="text-[var(--text-muted)] text-sm text-center">{t('stream.selectLayer', { defaultValue: 'Select a layer to edit its properties' })}</p>
        </CardBody>
      </Card>
    );
  }

  const handleUpdateTransform = async () => {
    try {
      await updateLayer(profile.name, scene.id, layer.id, {
        transform: {
          ...layer.transform,
          x,
          y,
          width,
          height,
        },
      });
      await reloadProfile();
    } catch (err) {
      toast.error(t('stream.updateFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to update: ${err instanceof Error ? err.message : String(err)}` }));
    }
  };

  const handleToggleVisibility = async () => {
    try {
      await updateLayer(profile.name, scene.id, layer.id, {
        visible: !layer.visible,
      });
      await reloadProfile();
    } catch (err) {
      toast.error(t('stream.updateFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to update: ${err instanceof Error ? err.message : String(err)}` }));
    }
  };

  const handleToggleLock = async () => {
    try {
      await updateLayer(profile.name, scene.id, layer.id, {
        locked: !layer.locked,
      });
      await reloadProfile();
    } catch (err) {
      toast.error(t('stream.updateFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to update: ${err instanceof Error ? err.message : String(err)}` }));
    }
  };

  const handleRemoveLayer = async () => {
    if (confirm(t('stream.confirmRemoveLayer', { defaultValue: 'Remove this layer from the scene?' }))) {
      try {
        await removeLayer(profile.name, scene.id, layer.id);
        selectLayer(null);
        await reloadProfile();
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
      await updateLayer(profile.name, scene.id, layer.id, {
        transform: { ...layer.transform, x: newX, y: newY, width: newWidth, height: newHeight },
      });
      await reloadProfile();
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
      await updateLayer(profile.name, scene.id, layer.id, {
        transform: { ...layer.transform, x: newX, y: newY },
      });
      await reloadProfile();
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
        <p className="text-xs text-[var(--text-muted)] truncate mt-1">{source?.name ?? t('stream.unknownSource', { defaultValue: 'Unknown Source' })}</p>
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
      </CardBody>
    </Card>
  );
}
