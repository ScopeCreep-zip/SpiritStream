/**
 * Audio Mixer Panel
 * Bottom panel with audio track controls
 */
import { useTranslation } from 'react-i18next';
import { Volume2, VolumeX } from 'lucide-react';
import { Card, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
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
      <CardBody className="py-3">
        <div className="flex items-end gap-4 overflow-x-auto pb-2">
          {/* Master volume */}
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

          {/* Divider */}
          <div className="w-px h-28 bg-border" />

          {/* Source tracks */}
          {scene.audioMixer.tracks.map((track) => (
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
          ))}

          {scene.audioMixer.tracks.length === 0 && (
            <p className="text-muted text-sm">{t('stream.noAudioTracks', { defaultValue: 'No audio tracks' })}</p>
          )}
        </div>
      </CardBody>
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
  // Convert 0-2 range to percentage for display
  const volumePercent = Math.round(volume * 100);

  // Handle vertical slider interaction via click/drag on the track
  const handleSliderInteraction = (e: React.MouseEvent<HTMLDivElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    const y = e.clientY - rect.top;
    const height = rect.height;
    // Invert: top = max (2), bottom = min (0)
    const newVolume = Math.max(0, Math.min(2, (1 - y / height) * 2));
    onVolumeChange(newVolume);
  };

  const handleDrag = (e: React.MouseEvent<HTMLDivElement>) => {
    if (e.buttons === 1) {
      handleSliderInteraction(e);
    }
  };

  return (
    <div className="flex flex-col items-center gap-2 min-w-[80px]">
      {/* Volume icon for master */}
      {isMaster && (
        <Volume2 className="w-4 h-4 text-primary" />
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
          className={`absolute bottom-0 left-0 right-0 rounded-full transition-all ${
            muted ? 'bg-muted' : 'bg-primary'
          }`}
          style={{ height: `${(volume / 2) * 100}%` }}
        />
        {/* Thumb indicator */}
        <div
          className={`absolute left-1/2 -translate-x-1/2 w-4 h-4 rounded-full border-2 shadow-md transition-all ${
            muted
              ? 'bg-muted border-muted'
              : 'bg-primary border-primary-foreground'
          }`}
          style={{ bottom: `calc(${(volume / 2) * 100}% - 8px)` }}
        />
      </div>

      {/* Volume indicator */}
      <span className="text-xs text-muted tabular-nums">{volumePercent}%</span>

      {/* Mute/Solo buttons - increased to 32px for better touch targets */}
      {!isMaster && (
        <div className="flex gap-1">
          <Button
            variant={muted ? 'destructive' : 'ghost'}
            size="sm"
            className="w-8 h-8 p-0 min-w-[32px]"
            onClick={() => onMuteToggle(!muted)}
            title={muted ? 'Unmute' : 'Mute'}
          >
            {muted ? (
              <VolumeX className="w-4 h-4" />
            ) : (
              <Volume2 className="w-4 h-4" />
            )}
          </Button>
          <Button
            variant={solo ? 'primary' : 'ghost'}
            size="sm"
            className="w-8 h-8 p-0 min-w-[32px]"
            onClick={() => onSoloToggle(!solo)}
            title={solo ? 'Unsolo' : 'Solo'}
          >
            <span className="text-sm font-bold">S</span>
          </Button>
        </div>
      )}

      {/* Label with tooltip for truncated text */}
      <span className="text-xs text-center truncate max-w-[80px]" title={label}>
        {label}
      </span>
    </div>
  );
}
