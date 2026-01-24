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
import { api } from '@/lib/backend/httpApi';
import { Go2rtcClient } from '@/lib/webrtc/go2rtcClient';

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
 * Live thumbnail preview for a source
 * Tries WebRTC first for smooth real-time preview, falls back to snapshot polling
 */
function SourceThumbnail({ sourceId, sourceType }: { sourceId: string; sourceType: Source['type'] }) {
  const [error, setError] = useState(false);
  const [loading, setLoading] = useState(true);
  const [useWebRTC, setUseWebRTC] = useState(true);
  const [snapshotUrl, setSnapshotUrl] = useState<string | null>(null);

  const videoRef = useRef<HTMLVideoElement>(null);
  const clientRef = useRef<Go2rtcClient | null>(null);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const errorCountRef = useRef(0);
  const webrtcAttemptedRef = useRef(false);

  // Only show preview for video sources
  const hasVideo = sourceType !== 'audioDevice';

  // WebRTC preview
  useEffect(() => {
    if (!hasVideo || !useWebRTC) return;

    let cancelled = false;
    webrtcAttemptedRef.current = true;

    const startWebRTC = async () => {
      try {
        // Request WebRTC stream from backend
        const result = await api.preview.startWebrtcPreview(sourceId);

        if (cancelled) return;

        // Create WebRTC client with the signaling URL
        const client = new Go2rtcClient({
          wsUrl: result.webrtcWsUrl,
          onConnectionStateChange: (state) => {
            if (state === 'connected') {
              setLoading(false);
              setError(false);
            } else if (state === 'failed' || state === 'closed') {
              // Fall back to snapshot polling
              console.log('[Preview] WebRTC failed, falling back to snapshots');
              setUseWebRTC(false);
            }
          },
          onError: () => {
            // Fall back to snapshot polling
            console.log('[Preview] WebRTC error, falling back to snapshots');
            setUseWebRTC(false);
          },
        });

        clientRef.current = client;

        const stream = await client.connect();

        if (cancelled) {
          client.disconnect();
          return;
        }

        if (videoRef.current) {
          videoRef.current.srcObject = stream;
        }
      } catch (err) {
        if (cancelled) return;
        console.log('[Preview] WebRTC not available, using snapshots:', err);
        setUseWebRTC(false);
      }
    };

    startWebRTC();

    return () => {
      cancelled = true;
      if (clientRef.current) {
        clientRef.current.disconnect();
        clientRef.current = null;
      }
      // Stop WebRTC preview on backend
      api.preview.stopWebrtcPreview(sourceId).catch(() => {});
    };
  }, [sourceId, hasVideo, useWebRTC]);

  // Snapshot fallback polling - fetch new frame every 200ms (5 fps for thumbnails)
  useEffect(() => {
    if (!hasVideo || useWebRTC) return;

    // Reset state for snapshot mode
    setError(false);
    setLoading(true);
    errorCountRef.current = 0;

    const fetchSnapshot = () => {
      // Generate URL with timestamp to prevent caching
      const url = api.preview.getSourceSnapshotUrl(sourceId, 80, 45, 8);
      setSnapshotUrl(url);
    };

    // Fetch first snapshot immediately
    fetchSnapshot();

    // Then poll at interval
    intervalRef.current = setInterval(fetchSnapshot, 200);

    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    };
  }, [sourceId, hasVideo, useWebRTC]);

  const handleLoad = useCallback(() => {
    setLoading(false);
    setError(false);
    errorCountRef.current = 0;
  }, []);

  const handleError = useCallback(() => {
    errorCountRef.current += 1;
    // Only show error after 3 consecutive failures (allow time for camera init)
    if (errorCountRef.current >= 3) {
      setLoading(false);
      setError(true);
      // Stop polling on persistent error
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
    }
  }, []);

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
      {useWebRTC ? (
        <video
          ref={videoRef}
          autoPlay
          muted
          playsInline
          className="w-full h-full object-cover"
          onLoadedData={handleLoad}
        />
      ) : (
        snapshotUrl && (
          <img
            src={snapshotUrl}
            alt="Preview"
            className="w-full h-full object-cover"
            onLoad={handleLoad}
            onError={handleError}
          />
        )
      )}
    </div>
  );
}

export function SourcesPanel({ profile, activeScene }: SourcesPanelProps) {
  const { t } = useTranslation();
  const { addLayer } = useSceneStore();
  const { removeSource } = useSourceStore();
  const { reloadProfile } = useProfileStore();
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
        await removeSource(profile.name, source.id);
        await reloadProfile();
        toast.success(t('stream.sourceRemoved', { name: source.name, defaultValue: `Removed ${source.name}` }));
      } catch (err) {
        toast.error(t('stream.sourceRemoveFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to remove source: ${err instanceof Error ? err.message : String(err)}` }));
      }
    }
  };

  return (
    <Card className="h-full flex flex-col">
      <CardHeader className="flex-shrink-0">
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
      <CardBody className="flex-1 overflow-y-auto p-2">
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
