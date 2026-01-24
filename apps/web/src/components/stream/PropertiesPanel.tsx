/**
 * Properties Panel
 * Right sidebar showing selected layer properties
 */
import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Eye, EyeOff, Lock, Unlock, Trash2 } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
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
          <p className="text-muted text-sm">{t('stream.noSceneSelected', { defaultValue: 'No scene selected' })}</p>
        </CardBody>
      </Card>
    );
  }

  if (!layer) {
    return (
      <Card className="h-full">
        <CardHeader>
          <CardTitle className="text-sm">{t('stream.properties', { defaultValue: 'Properties' })}</CardTitle>
        </CardHeader>
        <CardBody>
          <p className="text-muted text-sm">{t('stream.selectLayer', { defaultValue: 'Select a layer to edit its properties' })}</p>
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

  return (
    <Card className="h-full flex flex-col">
      <CardHeader className="flex-shrink-0">
        <div className="flex items-center justify-between">
          <CardTitle className="text-sm">{t('stream.properties', { defaultValue: 'Properties' })}</CardTitle>
          <div className="flex items-center gap-1">
            <Button
              variant="ghost"
              size="sm"
              onClick={handleToggleVisibility}
              title={layer.visible ? t('common.hide') : t('common.show')}
            >
              {layer.visible ? (
                <Eye className="w-4 h-4" />
              ) : (
                <EyeOff className="w-4 h-4 text-muted" />
              )}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={handleToggleLock}
              title={layer.locked ? t('stream.unlock', { defaultValue: 'Unlock' }) : t('stream.lock', { defaultValue: 'Lock' })}
            >
              {layer.locked ? (
                <Lock className="w-4 h-4 text-warning" />
              ) : (
                <Unlock className="w-4 h-4" />
              )}
            </Button>
          </div>
        </div>
        <p className="text-xs text-muted truncate">{source?.name ?? t('stream.unknownSource', { defaultValue: 'Unknown Source' })}</p>
      </CardHeader>

      <CardBody className="flex-1 overflow-y-auto space-y-4">
        {/* Lock indicator banner */}
        {layer.locked && (
          <div className="flex items-center gap-2 p-2 bg-warning/10 border border-warning/20 rounded text-warning text-sm">
            <Lock className="w-4 h-4 flex-shrink-0" />
            <span>{t('stream.layerLocked', { defaultValue: 'Layer is locked. Unlock to edit.' })}</span>
          </div>
        )}

        {/* Transform section */}
        <div>
          <h4 className="text-sm font-semibold text-[var(--text-secondary)] mb-3">{t('stream.transform', { defaultValue: 'Transform' })}</h4>
          <div className="space-y-3">
            {/* Position row */}
            <div className="flex items-center gap-2">
              <span className="text-xs text-muted w-12">{t('stream.position', { defaultValue: 'Position' })}</span>
              <div className="flex-1 grid grid-cols-2 gap-2">
                <Input
                  label="X"
                  type="number"
                  value={x}
                  onChange={(e) => setX(Number(e.target.value))}
                  onBlur={handleUpdateTransform}
                  disabled={layer.locked}
                  className={layer.locked ? 'opacity-50' : ''}
                />
                <Input
                  label="Y"
                  type="number"
                  value={y}
                  onChange={(e) => setY(Number(e.target.value))}
                  onBlur={handleUpdateTransform}
                  disabled={layer.locked}
                  className={layer.locked ? 'opacity-50' : ''}
                />
              </div>
            </div>
            {/* Size row */}
            <div className="flex items-center gap-2">
              <span className="text-xs text-muted w-12">{t('stream.size', { defaultValue: 'Size' })}</span>
              <div className="flex-1 grid grid-cols-2 gap-2">
                <Input
                  label={t('stream.width', { defaultValue: 'W' })}
                  type="number"
                  value={width}
                  onChange={(e) => setWidth(Number(e.target.value))}
                  onBlur={handleUpdateTransform}
                  disabled={layer.locked}
                  className={layer.locked ? 'opacity-50' : ''}
                />
                <Input
                  label={t('stream.height', { defaultValue: 'H' })}
                  type="number"
                  value={height}
                  onChange={(e) => setHeight(Number(e.target.value))}
                  onBlur={handleUpdateTransform}
                  disabled={layer.locked}
                  className={layer.locked ? 'opacity-50' : ''}
                />
              </div>
            </div>
          </div>
        </div>

        {/* Quick actions */}
        <div>
          <h4 className="text-sm font-semibold text-[var(--text-secondary)] mb-3">{t('stream.quickActions', { defaultValue: 'Quick Actions' })}</h4>
          <div className="space-y-1">
            <Button
              variant="ghost"
              size="sm"
              className="w-full justify-start"
              onClick={() => {
                setX(0);
                setY(0);
                setWidth(scene.canvasWidth);
                setHeight(scene.canvasHeight);
                handleUpdateTransform();
              }}
              disabled={layer.locked}
            >
              {t('stream.fillCanvas', { defaultValue: 'Fill Canvas' })}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              className="w-full justify-start"
              onClick={() => {
                setX(Math.round((scene.canvasWidth - width) / 2));
                setY(Math.round((scene.canvasHeight - height) / 2));
                handleUpdateTransform();
              }}
              disabled={layer.locked}
            >
              {t('stream.center', { defaultValue: 'Center' })}
            </Button>
          </div>
        </div>

        {/* Danger zone */}
        <div className="pt-4 border-t border-border">
          <Button
            variant="destructive"
            size="sm"
            className="w-full"
            onClick={handleRemoveLayer}
          >
            <Trash2 className="w-4 h-4 mr-2" />
            {t('stream.removeFromScene', { defaultValue: 'Remove from Scene' })}
          </Button>
        </div>
      </CardBody>
    </Card>
  );
}
