/**
 * Sources Panel
 * Left sidebar showing available sources with add functionality
 */
import { useState, useEffect, useRef, useCallback } from 'react';
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
 * Live thumbnail preview for a source using snapshot polling
 * Features:
 * - Prevents request accumulation by tracking pending state
 * - Uses exponential backoff on errors
 * - Automatic cleanup on unmount
 */
function SourceThumbnail({ sourceId, sourceType }: { sourceId: string; sourceType: Source['type'] }) {
  const [error, setError] = useState(false);
  const [loading, setLoading] = useState(true);
  const [snapshotUrl, setSnapshotUrl] = useState<string | null>(null);

  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const errorCountRef = useRef(0);
  const isPendingRef = useRef(false);
  const backoffDelayRef = useRef(500); // Start with 500ms, exponential backoff on errors
  const mountedRef = useRef(true);

  // Only show preview for video sources
  const hasVideo = sourceType !== 'audioDevice';

  // Snapshot polling with pending tracking and exponential backoff
  useEffect(() => {
    if (!hasVideo) return;

    // Reset state on mount
    mountedRef.current = true;
    setError(false);
    setLoading(true);
    errorCountRef.current = 0;
    backoffDelayRef.current = 500;
    isPendingRef.current = false;

    const scheduleNextFetch = () => {
      if (!mountedRef.current) return;
      timeoutRef.current = setTimeout(fetchSnapshot, backoffDelayRef.current);
    };

    const fetchSnapshot = () => {
      // Don't start new request if one is already pending
      if (isPendingRef.current || !mountedRef.current) {
        scheduleNextFetch();
        return;
      }

      // Generate URL with timestamp to prevent caching
      const url = api.preview.getSourceSnapshotUrl(sourceId, 192, 108, 3);
      isPendingRef.current = true;
      setSnapshotUrl(url);
    };

    // Fetch first snapshot immediately
    fetchSnapshot();

    return () => {
      mountedRef.current = false;
      isPendingRef.current = false;
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
        timeoutRef.current = null;
      }
    };
  }, [sourceId, hasVideo]);

  const handleLoad = useCallback(() => {
    if (!mountedRef.current) return;

    isPendingRef.current = false;
    setLoading(false);
    setError(false);
    errorCountRef.current = 0;
    // Reset backoff on success
    backoffDelayRef.current = 500;

    // Schedule next fetch
    if (timeoutRef.current) clearTimeout(timeoutRef.current);
    timeoutRef.current = setTimeout(() => {
      if (mountedRef.current) {
        const url = api.preview.getSourceSnapshotUrl(sourceId, 192, 108, 3);
        isPendingRef.current = true;
        setSnapshotUrl(url);
      }
    }, backoffDelayRef.current);
  }, [sourceId]);

  const handleError = useCallback(() => {
    if (!mountedRef.current) return;

    isPendingRef.current = false;
    errorCountRef.current += 1;

    // Exponential backoff on errors (max 5 seconds)
    backoffDelayRef.current = Math.min(backoffDelayRef.current * 1.5, 5000);

    // Only show error after 3 consecutive failures (allow time for camera init)
    if (errorCountRef.current >= 3) {
      setLoading(false);
      setError(true);
      // Stop polling on persistent error - don't accumulate requests
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
        timeoutRef.current = null;
      }
      return;
    }

    // Schedule retry with backoff
    if (timeoutRef.current) clearTimeout(timeoutRef.current);
    timeoutRef.current = setTimeout(() => {
      if (mountedRef.current) {
        const url = api.preview.getSourceSnapshotUrl(sourceId, 192, 108, 3);
        isPendingRef.current = true;
        setSnapshotUrl(url);
      }
    }, backoffDelayRef.current);
  }, [sourceId]);

  if (!hasVideo) {
    // Audio-only placeholder
    return (
      <div className="w-16 h-9 bg-[var(--bg-sunken)] rounded flex items-center justify-center flex-shrink-0">
        <Mic className="w-4 h-4 text-muted" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="w-16 h-9 bg-[var(--bg-sunken)] rounded flex items-center justify-center flex-shrink-0" title="Preview unavailable - check source configuration">
        <SourceIcon type={sourceType} />
      </div>
    );
  }

  return (
    <div className="relative w-16 h-9 bg-[var(--bg-sunken)] rounded overflow-hidden flex-shrink-0">
      {loading && (
        <div className="absolute inset-0 flex items-center justify-center">
          <div className="w-4 h-4 border-2 border-primary/30 border-t-primary rounded-full animate-spin" />
        </div>
      )}
      {snapshotUrl && (
        <img
          src={snapshotUrl}
          alt="Preview"
          className="w-full h-full object-cover"
          onLoad={handleLoad}
          onError={handleError}
        />
      )}
    </div>
  );
}

export function SourcesPanel({ profile, activeScene }: SourcesPanelProps) {
  const { t } = useTranslation();
  const { addLayer } = useSceneStore();
  const { removeSource } = useSourceStore();
  const { reloadProfile, removeCurrentSource } = useProfileStore();
  const [showAddModal, setShowAddModal] = useState(false);

  // Check if source is used in active scene
  const isSourceInScene = (sourceId: string) => {
    return activeScene?.layers.some((l) => l.sourceId === sourceId) ?? false;
  };

  // Check if source is visible in the scene (layer.visible)
  const isSourceVisible = (sourceId: string) => {
    const layer = activeScene?.layers.find((l) => l.sourceId === sourceId);
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
                    <div className="flex items-center gap-1">
                      <SourceIcon type={source.type} />
                      <span className="text-sm truncate" title={source.name}>{source.name}</span>
                    </div>
                  </div>
                  {inScene && (
                    isVisible ? (
                      <span title={t('stream.visibleInScene', { defaultValue: 'Visible in scene' })}>
                        <Eye className="w-4 h-4 text-primary" />
                      </span>
                    ) : (
                      <span title={t('stream.hiddenInScene', { defaultValue: 'Hidden in scene' })}>
                        <EyeOff className="w-4 h-4 text-muted" />
                      </span>
                    )
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
      />
    </Card>
  );
}
