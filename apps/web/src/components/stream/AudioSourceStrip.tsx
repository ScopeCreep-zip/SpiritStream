/**
 * AudioSourceStrip — Wrapper that creates stable handler references for a single audio source
 * Extracted from AudioMixerPanel
 *
 * Without this, inline arrow functions in .map() create new references every render,
 * defeating React.memo on UnifiedChannelStrip and causing unnecessary re-renders.
 */
import React, { useCallback } from 'react';
import { UnifiedChannelStrip } from './UnifiedChannelStrip';
import type { AudioFilter, Source, SourceAudioConfig } from '@/types/source';

export interface AudioSourceStripProps {
  sourceId: string;
  label: string;
  config: SourceAudioConfig;
  sources: Source[];
  captureError?: string;
  onVolumeChange: (sourceId: string, volume: number) => void;
  onMuteToggle: (sourceId: string, muted: boolean) => void;
  onSoloToggle: (sourceId: string, solo: boolean) => void;
  onFiltersChange: (sourceId: string, filters: AudioFilter[]) => void;
}

export const AudioSourceStrip = React.memo(function AudioSourceStrip({
  sourceId,
  label,
  config,
  sources,
  captureError,
  onVolumeChange,
  onMuteToggle,
  onSoloToggle,
  onFiltersChange,
}: AudioSourceStripProps) {
  const handleVolume = useCallback(
    (v: number) => onVolumeChange(sourceId, v),
    [sourceId, onVolumeChange]
  );
  const handleMute = useCallback(
    (m: boolean) => onMuteToggle(sourceId, m),
    [sourceId, onMuteToggle]
  );
  const handleSolo = useCallback(
    (s: boolean) => onSoloToggle(sourceId, s),
    [sourceId, onSoloToggle]
  );
  const handleFilters = useCallback(
    (f: AudioFilter[]) => onFiltersChange(sourceId, f),
    [sourceId, onFiltersChange]
  );

  return (
    <UnifiedChannelStrip
      trackId={sourceId}
      label={label}
      volume={config.volume}
      muted={config.muted}
      solo={config.solo}
      filters={config.audioFilters || []}
      monitoringType={config.monitoringType}
      balance={config.balance}
      availableSources={sources}
      captureError={captureError}
      onVolumeChange={handleVolume}
      onMuteToggle={handleMute}
      onSoloToggle={handleSolo}
      onFiltersChange={handleFilters}
    />
  );
});
