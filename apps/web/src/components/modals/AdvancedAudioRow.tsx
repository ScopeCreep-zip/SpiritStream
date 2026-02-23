/**
 * AdvancedAudioRow — single-source row for the Advanced Audio Properties table.
 * Extracted from AdvancedAudioProperties.tsx for modularity.
 */
import { useState } from 'react';
import type { Source, SourceAudioConfig, MonitoringType } from '@/types/source';
import { formatDb, linearToDb } from '@/utils/formatters';

const MAX_TRACKS = 6;

export interface AdvancedAudioRowProps {
  source: Source;
  config: SourceAudioConfig;
  onMonitoringChange: (sourceId: string, type: MonitoringType) => void;
  onTrackToggle: (sourceId: string, trackIdx: number, enabled: boolean) => void;
  onSyncOffsetChange: (sourceId: string, ms: number) => void;
  onBalanceChange: (sourceId: string, balance: number) => void;
}

export function AdvancedAudioRow({ source, config, onMonitoringChange, onTrackToggle, onSyncOffsetChange, onBalanceChange }: AdvancedAudioRowProps) {
  const [localSync, setLocalSync] = useState(String(config.syncOffsetMs));
  const [localBalance, setLocalBalance] = useState(config.balance);

  const volumeDb = formatDb(linearToDb(config.volume));

  function handleSyncBlur() {
    const ms = parseInt(localSync, 10);
    if (!isNaN(ms) && ms !== config.syncOffsetMs) {
      onSyncOffsetChange(source.id, ms);
    }
  }

  function handleBalanceCommit(val: number) {
    const rounded = Math.round(val * 100) / 100;
    setLocalBalance(rounded);
    onBalanceChange(source.id, rounded);
  }

  return (
    <tr className="border-b border-[var(--border-subtle)] hover:bg-[var(--bg-elevated)] transition-colors">
      {/* Source name */}
      <td className="py-2 px-2 text-[var(--text-primary)] font-medium truncate max-w-[160px]" title={source.name}>
        {source.name}
      </td>

      {/* Volume (dB) — read-only display */}
      <td className="py-2 px-2 text-center text-[var(--text-secondary)] tabular-nums">
        {volumeDb}
      </td>

      {/* Monitoring mode */}
      <td className="py-2 px-2 text-center">
        <select
          className="bg-[var(--bg-sunken)] border border-[var(--border-default)] rounded text-[10px] px-1 py-0.5 text-[var(--text-secondary)]"
          value={config.monitoringType}
          onChange={(e) => onMonitoringChange(source.id, e.target.value as MonitoringType)}
        >
          <option value="none">Off</option>
          <option value="monitorOnly">Monitor Only</option>
          <option value="monitorAndOutput">Monitor & Output</option>
        </select>
      </td>

      {/* Track checkboxes (T1-T6) */}
      {Array.from({ length: MAX_TRACKS }, (_, i) => {
        const isEnabled = (config.trackBitmask & (1 << i)) !== 0;
        return (
          <td key={i} className="py-2 px-1 text-center">
            <input
              type="checkbox"
              checked={isEnabled}
              onChange={(e) => onTrackToggle(source.id, i, e.target.checked)}
              className="w-3 h-3 accent-[var(--primary)]"
            />
          </td>
        );
      })}

      {/* Sync offset (ms) */}
      <td className="py-2 px-2 text-center">
        <input
          type="number"
          className="bg-[var(--bg-sunken)] border border-[var(--border-default)] rounded text-[10px] px-1 py-0.5 text-center w-16 text-[var(--text-secondary)] tabular-nums"
          value={localSync}
          onChange={(e) => setLocalSync(e.target.value)}
          onBlur={handleSyncBlur}
          onKeyDown={(e) => { if (e.key === 'Enter') handleSyncBlur(); }}
        />
      </td>

      {/* Balance slider */}
      <td className="py-2 px-2 text-center">
        <div className="flex items-center gap-1">
          <span className="text-[9px] text-[var(--text-muted)]">L</span>
          <input
            type="range"
            min={-100}
            max={100}
            value={Math.round(localBalance * 100)}
            onChange={(e) => setLocalBalance(parseInt(e.target.value, 10) / 100)}
            onMouseUp={() => handleBalanceCommit(localBalance)}
            onTouchEnd={() => handleBalanceCommit(localBalance)}
            className="w-16 h-1 accent-[var(--primary)]"
          />
          <span className="text-[9px] text-[var(--text-muted)]">R</span>
        </div>
      </td>
    </tr>
  );
}
