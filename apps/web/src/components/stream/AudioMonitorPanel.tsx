/**
 * Audio Monitor Panel
 * Real-time VU meters with peak hold and clip indicators
 */
import { useEffect, useRef, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Volume2 } from 'lucide-react';
import { Card, CardBody } from '@/components/ui/Card';
import { useAudioLevels } from '@/hooks/useAudioLevels';
import type { Profile, Scene } from '@/types/profile';

interface AudioMonitorPanelProps {
  profile: Profile;
  scene?: Scene;
}

interface AudioLevel {
  rms: number;      // 0-1 RMS level
  peak: number;     // 0-1 peak level
  clipping: boolean; // Whether clipping was detected
}

export function AudioMonitorPanel({ profile, scene }: AudioMonitorPanelProps) {
  const { t } = useTranslation();
  const { levels, isConnected } = useAudioLevels();

  // Get source name by ID
  const getSourceName = useCallback((sourceId: string) => {
    return profile.sources.find((s) => s.id === sourceId)?.name ?? 'Unknown';
  }, [profile.sources]);

  // Get active track IDs from the scene's audio mixer
  const trackIds = scene?.audioMixer.tracks.map((t) => t.sourceId) ?? [];

  if (!scene) {
    return (
      <Card>
        <CardBody className="py-3">
          <p className="text-muted text-sm text-center">
            {t('stream.noSceneSelected', { defaultValue: 'No scene selected' })}
          </p>
        </CardBody>
      </Card>
    );
  }

  return (
    <Card>
      <CardBody className="py-3 px-4">
        <div className="flex items-center gap-2 mb-3">
          <Volume2 className="w-4 h-4 text-muted" />
          <h4 className="text-sm font-medium text-[var(--text-secondary)]">
            {t('stream.audioMonitor', { defaultValue: 'Audio Monitor' })}
          </h4>
          {!isConnected && (
            <span className="text-xs text-yellow-500">
              {t('stream.audioMonitorDisconnected', { defaultValue: '(disconnected)' })}
            </span>
          )}
        </div>

        <div className="flex items-end gap-4 overflow-x-auto pb-2">
          {/* Per-track meters */}
          {trackIds.map((sourceId) => (
            <VUMeter
              key={sourceId}
              label={getSourceName(sourceId)}
              level={levels?.tracks[sourceId] ?? { rms: 0, peak: 0, clipping: false }}
            />
          ))}

          {/* Divider */}
          {trackIds.length > 0 && (
            <div className="w-px h-24 bg-[var(--border-default)] mx-2" />
          )}

          {/* Master meter */}
          <VUMeter
            label={t('stream.master', { defaultValue: 'Master' })}
            level={levels?.master ?? { rms: 0, peak: 0, clipping: false }}
            isMaster
          />
        </div>
      </CardBody>
    </Card>
  );
}

interface VUMeterProps {
  label: string;
  level: AudioLevel;
  isMaster?: boolean;
}

/**
 * VUMeter - Canvas-based audio level meter with peak hold
 */
function VUMeter({ label, level, isMaster = false }: VUMeterProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [peakHold, setPeakHold] = useState(0);
  const [isClipping, setIsClipping] = useState(false);
  const peakHoldTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const clipTimeoutRef = useRef<NodeJS.Timeout | null>(null);

  // Update peak hold with decay
  useEffect(() => {
    if (level.peak > peakHold) {
      setPeakHold(level.peak);

      // Clear existing timeout
      if (peakHoldTimeoutRef.current) {
        clearTimeout(peakHoldTimeoutRef.current);
      }

      // Hold peak for 1.5 seconds then decay
      peakHoldTimeoutRef.current = setTimeout(() => {
        const decayInterval = setInterval(() => {
          setPeakHold((prev) => {
            const newVal = prev - 0.02;
            if (newVal <= level.peak) {
              clearInterval(decayInterval);
              return level.peak;
            }
            return newVal;
          });
        }, 50);
      }, 1500);
    }

    return () => {
      if (peakHoldTimeoutRef.current) {
        clearTimeout(peakHoldTimeoutRef.current);
      }
    };
  }, [level.peak, peakHold]);

  // Handle clipping indicator
  useEffect(() => {
    if (level.clipping) {
      setIsClipping(true);

      if (clipTimeoutRef.current) {
        clearTimeout(clipTimeoutRef.current);
      }

      // Keep clip indicator on for 1 second after clipping stops
      clipTimeoutRef.current = setTimeout(() => {
        setIsClipping(false);
      }, 1000);
    }

    return () => {
      if (clipTimeoutRef.current) {
        clearTimeout(clipTimeoutRef.current);
      }
    };
  }, [level.clipping]);

  // Draw VU meter on canvas
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const width = canvas.width;
    const height = canvas.height;
    const barWidth = isMaster ? 16 : 12;
    const barX = (width - barWidth) / 2;

    // Clear canvas
    ctx.clearRect(0, 0, width, height);

    // Background
    ctx.fillStyle = 'var(--bg-sunken)';
    ctx.fillRect(barX, 0, barWidth, height);

    // Calculate level heights
    const rmsHeight = level.rms * height;
    const peakHeight = peakHold * height;

    // Draw gradient fill for RMS level
    const gradient = ctx.createLinearGradient(0, height, 0, 0);
    gradient.addColorStop(0, '#22c55e');     // Green at bottom
    gradient.addColorStop(0.6, '#22c55e');   // Green up to 60%
    gradient.addColorStop(0.8, '#eab308');   // Yellow 60-80%
    gradient.addColorStop(0.9, '#f97316');   // Orange 80-90%
    gradient.addColorStop(1, '#ef4444');     // Red at top

    ctx.fillStyle = gradient;
    ctx.fillRect(barX, height - rmsHeight, barWidth, rmsHeight);

    // Draw peak hold indicator
    if (peakHold > 0.01) {
      const peakY = height - peakHeight;
      ctx.fillStyle = peakHold > 0.9 ? '#ef4444' : peakHold > 0.7 ? '#eab308' : '#22c55e';
      ctx.fillRect(barX, peakY - 2, barWidth, 3);
    }

    // Draw scale markers (every 6dB = ~50% level)
    ctx.fillStyle = 'var(--text-muted)';
    ctx.globalAlpha = 0.3;
    for (let i = 0; i <= 4; i++) {
      const y = (i / 4) * height;
      ctx.fillRect(barX - 2, y, barWidth + 4, 1);
    }
    ctx.globalAlpha = 1;

  }, [level.rms, peakHold, isMaster]);

  return (
    <div className="flex flex-col items-center gap-1 min-w-[50px]">
      {/* Clip indicator */}
      <div
        className={`w-6 h-3 rounded-sm transition-colors ${
          isClipping
            ? 'bg-red-500 animate-pulse'
            : 'bg-[var(--bg-sunken)]'
        }`}
        title={isClipping ? 'Clipping!' : 'No clipping'}
      />

      {/* Canvas meter */}
      <canvas
        ref={canvasRef}
        width={isMaster ? 24 : 20}
        height={80}
        className="rounded"
      />

      {/* dB label */}
      <span className="text-[9px] text-muted tabular-nums">
        {level.rms > 0.001 ? `${(20 * Math.log10(level.rms)).toFixed(0)}dB` : '-∞'}
      </span>

      {/* Track label */}
      <span className={`text-xs text-center truncate max-w-[60px] ${isMaster ? 'font-medium' : ''}`}>
        {label}
      </span>
    </div>
  );
}
