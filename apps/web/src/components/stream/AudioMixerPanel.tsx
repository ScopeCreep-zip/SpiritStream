/**
 * Audio Mixer Panel
 * Bottom panel with audio track controls
 */
import { useState, useEffect, useRef, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Volume2, VolumeX, Plus } from 'lucide-react';
import { Card, CardBody } from '@/components/ui/Card';
import { AddSourceModal } from '@/components/modals/AddSourceModal';
import type { Profile, Scene } from '@/types/profile';
import { useSceneStore } from '@/stores/sceneStore';
import { useProfileStore } from '@/stores/profileStore';
import { toast } from '@/hooks/useToast';

interface AudioMixerPanelProps {
  profile: Profile;
  scene?: Scene;
}

export function AudioMixerPanel({ profile, scene }: AudioMixerPanelProps) {
  const { t } = useTranslation();
  const { setTrackVolume, setTrackMuted, setTrackSolo, setMasterVolume } = useSceneStore();
  const { reloadProfile } = useProfileStore();
  const [showAddModal, setShowAddModal] = useState(false);

  if (!scene) {
    return (
      <Card>
        <CardBody className="py-4 text-center">
          <p className="text-muted text-sm">{t('stream.noSceneSelected', { defaultValue: 'No scene selected' })}</p>
        </CardBody>
      </Card>
    );
  }

  const getSourceName = (sourceId: string) => {
    return profile.sources.find((s) => s.id === sourceId)?.name ?? t('stream.unknownSource', { defaultValue: 'Unknown' });
  };

  const handleVolumeChange = async (sourceId: string, volume: number) => {
    try {
      await setTrackVolume(profile.name, scene.id, sourceId, volume);
      await reloadProfile();
    } catch (err) {
      toast.error(t('stream.volumeFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to set volume: ${err instanceof Error ? err.message : String(err)}` }));
    }
  };

  const handleMuteToggle = async (sourceId: string, muted: boolean) => {
    try {
      await setTrackMuted(profile.name, scene.id, sourceId, muted);
      await reloadProfile();
    } catch (err) {
      toast.error(t('stream.muteFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to toggle mute: ${err instanceof Error ? err.message : String(err)}` }));
    }
  };

  const handleSoloToggle = async (sourceId: string, solo: boolean) => {
    try {
      await setTrackSolo(profile.name, scene.id, sourceId, solo);
      await reloadProfile();
    } catch (err) {
      toast.error(t('stream.soloFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to toggle solo: ${err instanceof Error ? err.message : String(err)}` }));
    }
  };

  const handleMasterVolumeChange = async (volume: number) => {
    try {
      await setMasterVolume(profile.name, scene.id, volume);
      await reloadProfile();
    } catch (err) {
      toast.error(t('stream.masterVolumeFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to set master volume: ${err instanceof Error ? err.message : String(err)}` }));
    }
  };

  return (
    <Card>
      <CardBody style={{ padding: '12px 16px' }}>
        <div className="flex items-stretch overflow-x-auto pb-2">
          {/* INPUT SECTION */}
          <div className="flex flex-col min-w-0 flex-1">
            <div className="flex items-center gap-2 mb-3">
              <h4 className="text-sm font-medium text-[var(--text-secondary)]">
                {t('stream.input', { defaultValue: 'Input' })}
              </h4>
              <button
                type="button"
                className="w-6 h-6 flex items-center justify-center rounded bg-[var(--bg-sunken)] hover:bg-[var(--bg-elevated)] text-[var(--text-muted)] hover:text-[var(--text-secondary)] transition-colors"
                onClick={() => setShowAddModal(true)}
                title={t('stream.addAudioSource', { defaultValue: 'Add Audio Source' })}
              >
                <Plus className="w-4 h-4" />
              </button>
            </div>
            <div className="flex items-start gap-4 flex-1">
              {scene.audioMixer.tracks.length > 0 ? (
                scene.audioMixer.tracks.map((track) => (
                  <AudioTrackControl
                    key={track.sourceId}
                    label={getSourceName(track.sourceId)}
                    volume={track.volume}
                    muted={track.muted}
                    solo={track.solo}
                    onVolumeChange={(v) => handleVolumeChange(track.sourceId, v)}
                    onMuteToggle={(m) => handleMuteToggle(track.sourceId, m)}
                    onSoloToggle={(s) => handleSoloToggle(track.sourceId, s)}
                  />
                ))
              ) : (
                <div className="flex items-center justify-center h-full min-h-[140px] px-4">
                  <p className="text-[var(--text-muted)] text-sm">
                    {t('stream.noAudioTracks', { defaultValue: 'No audio tracks' })}
                  </p>
                </div>
              )}
            </div>
          </div>

          {/* Divider */}
          <div className="flex flex-col mx-4 py-1">
            <div className="w-px flex-1 bg-[var(--border-default)]" />
          </div>

          {/* OUTPUT SECTION */}
          <div className="flex flex-col">
            <h4 className="text-sm font-medium text-[var(--text-secondary)] mb-3">
              {t('stream.output', { defaultValue: 'Output' })}
            </h4>
            <div className="flex items-start gap-4 flex-1">
              <AudioTrackControl
                label={t('stream.master', { defaultValue: 'Master' })}
                volume={scene.audioMixer.masterVolume}
                muted={false}
                solo={false}
                isMaster
                onVolumeChange={handleMasterVolumeChange}
                onMuteToggle={() => {}}
                onSoloToggle={() => {}}
              />
            </div>
          </div>
        </div>
      </CardBody>

      {/* Add Audio Source Modal */}
      <AddSourceModal
        open={showAddModal}
        onClose={() => setShowAddModal(false)}
        profileName={profile.name}
        filterType="audioDevice"
      />
    </Card>
  );
}

interface AudioTrackControlProps {
  label: string;
  volume: number;
  muted: boolean;
  solo: boolean;
  isMaster?: boolean;
  onVolumeChange: (volume: number) => void;
  onMuteToggle: (muted: boolean) => void;
  onSoloToggle: (solo: boolean) => void;
}

function AudioTrackControl({
  label,
  volume,
  muted,
  solo,
  isMaster,
  onVolumeChange,
  onMuteToggle,
  onSoloToggle,
}: AudioTrackControlProps) {
  // Local volume state for responsive UI during drag
  const [localVolume, setLocalVolume] = useState(volume);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isDraggingRef = useRef(false);

  // Sync local volume when prop changes (from server)
  useEffect(() => {
    if (!isDraggingRef.current) {
      setLocalVolume(volume);
    }
  }, [volume]);

  // Debounced save to server
  const debouncedSave = useCallback((newVolume: number) => {
    if (debounceRef.current) {
      clearTimeout(debounceRef.current);
    }
    debounceRef.current = setTimeout(() => {
      onVolumeChange(newVolume);
      isDraggingRef.current = false;
    }, 300); // Save 300ms after last change
  }, [onVolumeChange]);

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
      }
    };
  }, []);

  // Convert 0-1 range to percentage for display (0% to 100%)
  const volumePercent = Math.round(localVolume * 100);

  // Handle vertical slider interaction via click/drag on the track
  const handleSliderInteraction = (e: React.MouseEvent<HTMLDivElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const y = e.clientY - rect.top;
    const height = rect.height;
    // Invert: top = max (1.0), bottom = min (0)
    const newVolume = Math.max(0, Math.min(1, 1 - y / height));
    isDraggingRef.current = true;
    setLocalVolume(newVolume);
    debouncedSave(newVolume);
  };

  const handleDrag = (e: React.MouseEvent<HTMLDivElement>) => {
    if (e.buttons === 1) {
      handleSliderInteraction(e);
    }
  };

  return (
    <div className="flex flex-col items-center gap-2 min-w-[80px]">
      {/* Mute/Mono buttons above slider (non-master tracks) */}
      {!isMaster ? (
        <div className="flex gap-1 mb-2">
          <button
            type="button"
            className={`w-7 h-7 rounded-md flex items-center justify-center transition-all border ${
              muted
                ? 'bg-destructive/20 border-destructive text-destructive'
                : 'bg-[var(--bg-sunken)] border-transparent text-[var(--text-muted)] hover:bg-[var(--bg-elevated)] hover:text-[var(--text-secondary)]'
            }`}
            onClick={() => onMuteToggle(!muted)}
            title={muted ? 'Unmute' : 'Mute'}
          >
            {muted ? (
              <VolumeX className="w-4 h-4" />
            ) : (
              <Volume2 className="w-4 h-4" />
            )}
          </button>
          <button
            type="button"
            className={`w-7 h-7 rounded-md flex items-center justify-center transition-all border ${
              solo
                ? 'bg-primary/20 border-primary text-primary'
                : 'bg-[var(--bg-sunken)] border-transparent text-[var(--text-muted)] hover:bg-[var(--bg-elevated)] hover:text-[var(--text-secondary)]'
            }`}
            onClick={() => onSoloToggle(!solo)}
            title={solo ? 'Disable Mono' : 'Enable Mono'}
          >
            {/* Mono icon: single filled circle representing one channel */}
            <svg className="w-4 h-4" viewBox="0 0 24 24" fill="currentColor">
              <circle cx="12" cy="12" r="6" />
            </svg>
          </button>
        </div>
      ) : (
        /* Volume icon for master */
        <div className="h-7 mb-2 flex items-center justify-center">
          <Volume2 className="w-5 h-5 text-primary" />
        </div>
      )}

      {/* Volume slider (vertical) - custom implementation for better UX */}
      <div
        className="relative h-24 w-3 bg-[var(--bg-sunken)] rounded-full cursor-pointer select-none"
        onClick={handleSliderInteraction}
        onMouseMove={handleDrag}
        title={`${volumePercent}%`}
      >
        {/* Fill bar */}
        <div
          className={`absolute bottom-0 left-0 right-0 rounded-full ${
            muted ? 'bg-muted' : 'bg-primary'
          }`}
          style={{ height: `${localVolume * 100}%` }}
        />
        {/* Thumb indicator */}
        <div
          className={`absolute left-1/2 -translate-x-1/2 w-4 h-4 rounded-full border-2 shadow-md ${
            muted
              ? 'bg-muted border-muted'
              : 'bg-primary border-primary-foreground'
          }`}
          style={{ bottom: `calc(${localVolume * 100}% - 8px)` }}
        />
      </div>

      {/* Volume indicator */}
      <span className="text-xs text-muted tabular-nums">{volumePercent}%</span>

      {/* Label */}
      <span className="text-xs text-center break-words max-w-[80px]">
        {label}
      </span>
    </div>
  );
}
