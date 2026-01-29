/**
 * Sources Panel
 * Left sidebar showing available sources with add functionality
 */
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Plus,
  Radio,
  Film,
  Monitor,
  Camera,
  Usb,
  Mic,
  Trash2,
  Eye,
  EyeOff,
} from 'lucide-react';
import { Card, CardHeader, CardTitle, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { AddSourceModal } from '@/components/modals/AddSourceModal';
import type { Profile, Scene, Source } from '@/types/profile';
import { useSceneStore } from '@/stores/sceneStore';
import { useSourceStore } from '@/stores/sourceStore';
import { useProfileStore } from '@/stores/profileStore';
import { toast } from '@/hooks/useToast';
import { api } from '@/lib/backend';
import { useWebRTCPreview } from '@/hooks/useWebRTCPreview';

interface SourcesPanelProps {
  profile: Profile;
  activeScene?: Scene;
}

const SourceIcon = ({ type }: { type: Source['type'] }) => {
  const iconClass = 'w-4 h-4';
  switch (type) {
    case 'rtmp':
      return <Radio className={iconClass} />;
    case 'mediaFile':
      return <Film className={iconClass} />;
    case 'screenCapture':
      return <Monitor className={iconClass} />;
    case 'camera':
      return <Camera className={iconClass} />;
    case 'captureCard':
      return <Usb className={iconClass} />;
    case 'audioDevice':
      return <Mic className={iconClass} />;
    default:
      return null;
  }
};

/**
 * Live thumbnail preview for a source using WebRTC
 * Uses the same WebRTC system as the scene canvas for consistency
 */
function SourceThumbnail({ sourceId, sourceType }: { sourceId: string; sourceType: Source['type'] }) {
  const { status, videoRef, retry } = useWebRTCPreview(sourceId);

  // Only show preview for video sources
  const hasVideo = sourceType !== 'audioDevice';

  if (!hasVideo) {
    // Audio-only placeholder
    return (
      <div className="w-16 h-9 bg-[var(--bg-sunken)] rounded flex items-center justify-center flex-shrink-0">
        <Mic className="w-4 h-4 text-muted" />
      </div>
    );
  }

  return (
    <div className="relative w-16 h-9 bg-[var(--bg-sunken)] rounded overflow-hidden flex-shrink-0">
      {/* Video element for WebRTC */}
      <video
        ref={videoRef}
        autoPlay
        muted
        playsInline
        className="w-full h-full object-cover"
        style={{ display: status === 'playing' ? 'block' : 'none' }}
      />

      {/* Loading state */}
      {(status === 'loading' || status === 'connecting') && (
        <div className="absolute inset-0 flex items-center justify-center">
          <div className="w-4 h-4 border-2 border-primary/30 border-t-primary rounded-full animate-spin" />
        </div>
      )}

      {/* Error/Unavailable state - show icon with retry on click */}
      {(status === 'error' || status === 'unavailable') && (
        <div
          className="absolute inset-0 flex items-center justify-center cursor-pointer hover:bg-muted/20 transition-colors"
          onClick={(e) => {
            e.stopPropagation();
            retry();
          }}
          title="Click to retry preview"
        >
          <SourceIcon type={sourceType} />
        </div>
      )}
    </div>
  );
}

export function SourcesPanel({ profile, activeScene }: SourcesPanelProps) {
  const { t } = useTranslation();
  const { addLayer, updateLayer } = useSceneStore();
  const { removeSource } = useSourceStore();
  const { reloadProfile, removeCurrentSource } = useProfileStore();
  const [showAddModal, setShowAddModal] = useState(false);

  // Check if source is used in active scene
  const isSourceInScene = (sourceId: string) => {
    return activeScene?.layers.some((l) => l.sourceId === sourceId) ?? false;
  };

  // Get the layer for a source in the active scene
  const getLayerForSource = (sourceId: string) => {
    return activeScene?.layers.find((l) => l.sourceId === sourceId);
  };

  // Check if source is visible in the scene (layer.visible)
  const isSourceVisible = (sourceId: string) => {
    const layer = getLayerForSource(sourceId);
    return layer?.visible ?? false;
  };

  const handleAddToScene = async (source: Source) => {
    if (!activeScene) {
      toast.error(t('stream.noActiveScene', { defaultValue: 'No active scene' }));
      return;
    }

    try {
      await addLayer(profile.name, activeScene.id, source.id);
      await reloadProfile();
      toast.success(t('stream.sourceAdded', { name: source.name, defaultValue: `Added ${source.name} to scene` }));
    } catch (err) {
      toast.error(t('stream.sourceAddFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to add source: ${err instanceof Error ? err.message : String(err)}` }));
    }
  };

  const handleRemoveSource = async (source: Source) => {
    if (confirm(t('stream.confirmRemoveSource', { name: source.name, defaultValue: `Remove "${source.name}" from profile? This will also remove it from all scenes.` }))) {
      try {
        // Stop any running preview for this source first
        try {
          await api.preview.stopSourcePreview(source.id);
        } catch {
          // Ignore errors - preview may not be running
        }

        await removeSource(profile.name, source.id);
        // Update local state without reloading entire profile to avoid overwriting local edits
        removeCurrentSource(source.id);
        toast.success(t('stream.sourceRemoved', { name: source.name, defaultValue: `Removed ${source.name}` }));
      } catch (err) {
        toast.error(t('stream.sourceRemoveFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to remove source: ${err instanceof Error ? err.message : String(err)}` }));
      }
    }
  };

  const handleToggleVisibility = async (sourceId: string) => {
    if (!activeScene) return;

    const layer = getLayerForSource(sourceId);
    if (!layer) return;

    try {
      await updateLayer(profile.name, activeScene.id, layer.id, { visible: !layer.visible });
      await reloadProfile();
    } catch (err) {
      toast.error(t('stream.visibilityToggleFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to toggle visibility: ${err instanceof Error ? err.message : String(err)}` }));
    }
  };

  return (
    <Card className="h-full flex flex-col">
      <CardHeader className="flex-shrink-0" style={{ padding: '12px 16px' }}>
        <div className="flex items-center justify-between">
          <CardTitle className="text-sm">{t('stream.sources', { defaultValue: 'Sources' })}</CardTitle>
          <Button
            variant="ghost"
            size="sm"
            className="min-w-[36px] min-h-[36px]"
            onClick={() => setShowAddModal(true)}
            title={t('stream.addSource', { defaultValue: 'Add Source' })}
          >
            <Plus className="w-4 h-4" />
          </Button>
        </div>
      </CardHeader>
      <CardBody className="flex-1 overflow-y-auto" style={{ padding: '12px' }}>
        {profile.sources.length === 0 ? (
          <div className="text-center text-muted text-sm py-8">
            <p>{t('stream.noSources', { defaultValue: 'No sources' })}</p>
            <Button
              variant="ghost"
              size="sm"
              className="mt-2"
              onClick={() => setShowAddModal(true)}
            >
              <Plus className="w-4 h-4 mr-1" />
              {t('stream.addSource', { defaultValue: 'Add Source' })}
            </Button>
          </div>
        ) : (
          <div className="space-y-1">
            {profile.sources.map((source) => {
              const inScene = isSourceInScene(source.id);
              const isVisible = isSourceVisible(source.id);
              return (
                <div
                  key={source.id}
                  className={`flex items-center gap-2 p-2 rounded group transition-colors ${
                    inScene
                      ? 'opacity-60 cursor-default bg-muted/20'
                      : 'cursor-pointer hover:bg-muted/50'
                  }`}
                  onClick={() => !inScene && handleAddToScene(source)}
                  title={inScene ? t('stream.alreadyInScene', { defaultValue: 'Already in scene' }) : t('stream.clickToAdd', { defaultValue: 'Click to add to scene' })}
                >
                  {/* Live thumbnail preview */}
                  <SourceThumbnail sourceId={source.id} sourceType={source.type} />
                  <div className="flex-1 min-w-0">
                    <div className="flex items-start gap-1">
                      <SourceIcon type={source.type} />
                      <span className="text-sm break-words">{source.name}</span>
                    </div>
                  </div>
                  {inScene && (
                    <button
                      className="p-1.5 rounded hover:bg-muted/50 transition-colors min-w-[28px] min-h-[28px] flex items-center justify-center"
                      onClick={(e) => {
                        e.stopPropagation();
                        handleToggleVisibility(source.id);
                      }}
                      title={isVisible ? t('stream.hideInScene', { defaultValue: 'Hide in scene' }) : t('stream.showInScene', { defaultValue: 'Show in scene' })}
                    >
                      {isVisible ? (
                        <Eye className="w-4 h-4 text-primary" />
                      ) : (
                        <EyeOff className="w-4 h-4 text-muted" />
                      )}
                    </button>
                  )}
                  <button
                    className="opacity-40 group-hover:opacity-100 p-1.5 hover:bg-destructive/20 rounded transition-opacity min-w-[28px] min-h-[28px] flex items-center justify-center"
                    onClick={(e) => {
                      e.stopPropagation();
                      handleRemoveSource(source);
                    }}
                    title={t('stream.removeSource', { defaultValue: 'Remove source' })}
                  >
                    <Trash2 className="w-4 h-4 text-destructive" />
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </CardBody>

      {/* Add Source Modal */}
      <AddSourceModal
        open={showAddModal}
        onClose={() => setShowAddModal(false)}
        profileName={profile.name}
        excludeTypes={['audioDevice']}
      />
    </Card>
  );
}
