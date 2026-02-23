/**
 * Advanced Audio Properties Modal
 * OBS-parity "Advanced Audio Properties" dialog.
 * Table layout showing all audio sources with monitoring, track routing, sync offset, and balance.
 */
import { useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { Button } from '@/components/ui/Button';
import { api } from '@/lib/backend';
import { useProfileStore } from '@/stores/profileStore';
import { toast } from '@/hooks/useToast';
import { sourceHasAudio, createDefaultSourceAudioConfig } from '@/types/source';
import type { MonitoringType } from '@/types/source';
import type { Profile } from '@/types/profile';
import { useShallow } from 'zustand/shallow';
import { AdvancedAudioRow } from './AdvancedAudioRow';

const MAX_TRACKS = 6;

interface AdvancedAudioPropertiesProps {
  open: boolean;
  onClose: () => void;
  profile: Profile;
}

export function AdvancedAudioProperties({ open, onClose, profile }: AdvancedAudioPropertiesProps) {
  const { t } = useTranslation();
  const { setSourceAudioConfig } = useProfileStore(
    useShallow(s => ({ setSourceAudioConfig: s.setSourceAudioConfig }))
  );

  const audioSources = useMemo(() => {
    return profile.sources.filter(s => sourceHasAudio(s));
  }, [profile.sources]);

  const sourceAudioConfigs = profile.sourceAudioConfigs ?? {};

  const handleMonitoringChange = useCallback(async (sourceId: string, monitoringType: MonitoringType) => {
    try {
      setSourceAudioConfig(sourceId, { monitoringType });
      await api.invoke('set_source_monitoring', {
        profileName: profile.name,
        sourceId,
        monitoringType,
      });
    } catch (err) {
      toast.error(`Failed to set monitoring: ${err instanceof Error ? err.message : String(err)}`);
    }
  }, [profile.name, setSourceAudioConfig]);

  const handleTrackToggle = useCallback(async (sourceId: string, trackIdx: number, enabled: boolean) => {
    const config = sourceAudioConfigs[sourceId] ?? createDefaultSourceAudioConfig();
    const bit = 1 << trackIdx;
    const newBitmask = enabled ? (config.trackBitmask | bit) : (config.trackBitmask & ~bit);
    try {
      setSourceAudioConfig(sourceId, { trackBitmask: newBitmask });
      await api.invoke('set_source_track_bitmask', {
        profileName: profile.name,
        sourceId,
        trackBitmask: newBitmask,
      });
    } catch (err) {
      toast.error(`Failed to set track routing: ${err instanceof Error ? err.message : String(err)}`);
    }
  }, [profile.name, sourceAudioConfigs, setSourceAudioConfig]);

  const handleSyncOffsetChange = useCallback(async (sourceId: string, syncOffsetMs: number) => {
    try {
      setSourceAudioConfig(sourceId, { syncOffsetMs });
      await api.invoke('set_source_sync_offset', {
        profileName: profile.name,
        sourceId,
        syncOffsetMs,
      });
    } catch (err) {
      toast.error(`Failed to set sync offset: ${err instanceof Error ? err.message : String(err)}`);
    }
  }, [profile.name, setSourceAudioConfig]);

  const handleBalanceChange = useCallback(async (sourceId: string, balance: number) => {
    try {
      setSourceAudioConfig(sourceId, { balance });
      await api.invoke('set_source_balance', {
        profileName: profile.name,
        sourceId,
        balance,
      });
    } catch (err) {
      toast.error(`Failed to set balance: ${err instanceof Error ? err.message : String(err)}`);
    }
  }, [profile.name, setSourceAudioConfig]);

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={t('audio.advancedProperties', { defaultValue: 'Advanced Audio Properties' })}
      maxWidth="900px"
    >
      <div className="p-4 overflow-x-auto">
        {audioSources.length === 0 ? (
          <p className="text-[var(--text-muted)] text-sm text-center py-8">
            {t('audio.noAudioSources', { defaultValue: 'No audio sources in this profile.' })}
          </p>
        ) : (
          <table className="w-full text-xs">
            <thead>
              <tr className="text-[var(--text-muted)] border-b border-[var(--border-default)]">
                <th className="text-left py-2 px-2 font-medium">
                  {t('audio.source', { defaultValue: 'Source' })}
                </th>
                <th className="text-center py-2 px-2 font-medium">
                  {t('audio.volumeDb', { defaultValue: 'Vol (dB)' })}
                </th>
                <th className="text-center py-2 px-2 font-medium">
                  {t('audio.monitor', { defaultValue: 'Monitor' })}
                </th>
                {Array.from({ length: MAX_TRACKS }, (_, i) => (
                  <th key={i} className="text-center py-2 px-1 font-medium w-8">
                    T{i + 1}
                  </th>
                ))}
                <th className="text-center py-2 px-2 font-medium">
                  {t('audio.syncMs', { defaultValue: 'Sync (ms)' })}
                </th>
                <th className="text-center py-2 px-2 font-medium">
                  {t('audio.balance', { defaultValue: 'Balance' })}
                </th>
              </tr>
            </thead>
            <tbody>
              {audioSources.map((source) => {
                const config = sourceAudioConfigs[source.id] ?? createDefaultSourceAudioConfig();
                return (
                  <AdvancedAudioRow
                    key={source.id}
                    source={source}
                    config={config}
                    onMonitoringChange={handleMonitoringChange}
                    onTrackToggle={handleTrackToggle}
                    onSyncOffsetChange={handleSyncOffsetChange}
                    onBalanceChange={handleBalanceChange}
                  />
                );
              })}
            </tbody>
          </table>
        )}
      </div>
      <div className="flex justify-end p-4 border-t border-[var(--border-default)]">
        <Button variant="secondary" onClick={onClose}>
          {t('common.close', { defaultValue: 'Close' })}
        </Button>
      </div>
    </Modal>
  );
}
