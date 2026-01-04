import { Pencil, Copy, Trash2, Cpu, Monitor, Gauge, Film, Volume2, Settings2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/cn';
import { Card, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { StreamStatus } from '@/components/ui/StreamStatus';
import type { OutputGroup } from '@/types/profile';
import type { StreamStatusType } from '@/types/stream';

export interface EncoderCardProps {
  group: OutputGroup;
  status: StreamStatusType;
  onEdit?: () => void;
  onDuplicate: () => void;
  onRemove: () => void;
  className?: string;
}

/**
 * Get a human-readable label for an encoder codec
 */
function getEncoderLabel(codec: string): { label: string; type: 'software' | 'hardware' | 'passthrough' } {
  if (codec === 'copy') {
    return { label: 'Passthrough', type: 'passthrough' };
  }
  const encoders: Record<string, { label: string; type: 'software' | 'hardware' }> = {
    libx264: { label: 'x264', type: 'software' },
    libx265: { label: 'x265', type: 'software' },
    h264_nvenc: { label: 'NVENC', type: 'hardware' },
    hevc_nvenc: { label: 'NVENC HEVC', type: 'hardware' },
    h264_videotoolbox: { label: 'VideoToolbox', type: 'hardware' },
    hevc_videotoolbox: { label: 'VideoToolbox HEVC', type: 'hardware' },
    h264_qsv: { label: 'QuickSync', type: 'hardware' },
    hevc_qsv: { label: 'QuickSync HEVC', type: 'hardware' },
    h264_amf: { label: 'AMF', type: 'hardware' },
    hevc_amf: { label: 'AMF HEVC', type: 'hardware' },
  };
  return encoders[codec] || { label: codec, type: 'software' };
}

/**
 * Get a human-readable label for an encoder preset
 */
function getPresetLabel(preset: string | undefined): string {
  if (!preset) return '—';
  const presets: Record<string, string> = {
    ultrafast: 'Ultrafast',
    superfast: 'Superfast',
    veryfast: 'Very Fast',
    faster: 'Faster',
    fast: 'Fast',
    medium: 'Medium',
    slow: 'Slow',
    slower: 'Slower',
    veryslow: 'Very Slow',
    // NVENC presets
    p1: 'P1 (Fastest)',
    p2: 'P2',
    p3: 'P3',
    p4: 'P4 (Balanced)',
    p5: 'P5',
    p6: 'P6',
    p7: 'P7 (Highest Quality)',
  };
  return presets[preset] || preset;
}

export function EncoderCard({
  group,
  status,
  onEdit,
  onDuplicate,
  onRemove,
  className,
}: EncoderCardProps) {
  const { t } = useTranslation();
  const tDynamic = t as (key: string, options?: { defaultValue?: string }) => string;

  const isPassthrough = group.video.codec === 'copy' && group.audio.codec === 'copy';
  const isDefaultGroup = group.isDefault === true;

  const encoder = getEncoderLabel(group.video.codec);
  const resolution = isPassthrough ? 'Source' : `${group.video.width}×${group.video.height}`;
  const bitrate = isPassthrough ? 'Source' : group.video.bitrate;
  const fps = isPassthrough ? 'Source' : `${group.video.fps} fps`;
  const preset = getPresetLabel(group.video.preset);
  const profile = group.video.profile?.toUpperCase() || '—';
  const audioSummary = isPassthrough ? 'Source' : `${group.audio.codec.toUpperCase()} @ ${group.audio.bitrate}`;

  return (
    <Card className={cn('transition-all duration-150', className)}>
      <CardBody>
        {/* Header Row */}
        <div className="flex items-start justify-between" style={{ marginBottom: '16px' }}>
          <div className="flex items-center" style={{ gap: '12px' }}>
            {/* Encoder Icon */}
            <div
              className={cn(
                'w-10 h-10 rounded-lg flex items-center justify-center',
                encoder.type === 'passthrough'
                  ? 'bg-[var(--bg-muted)] text-[var(--text-secondary)]'
                  : encoder.type === 'hardware'
                    ? 'bg-[var(--success-subtle)] text-[var(--success-text)]'
                    : 'bg-[var(--primary-subtle)] text-[var(--primary)]'
              )}
            >
              <Cpu className="w-5 h-5" />
            </div>
            <div>
              <h3 className="font-semibold text-[var(--text-primary)]">
                {group.name || tDynamic('encoder.defaultEncoderName', { defaultValue: 'Encoder' })}
                {isDefaultGroup && (
                  <span className="ml-2 text-xs font-normal text-[var(--text-tertiary)]">
                    ({tDynamic('encoder.readonly', { defaultValue: 'Read-only' })})
                  </span>
                )}
              </h3>
              <p className="text-sm text-[var(--text-secondary)]">
                {encoder.label}
                {encoder.type !== 'passthrough' && (
                  <span className="text-[var(--text-tertiary)]">
                    {' '}
                    ({encoder.type === 'hardware' ? 'Hardware' : 'Software'})
                  </span>
                )}
              </p>
            </div>
          </div>
          <div className="flex items-center" style={{ gap: '8px' }}>
            <StreamStatus status={status} />
          </div>
        </div>

        {/* Video Settings Grid */}
        <div
          className="grid gap-4 py-4 border-t border-b border-[var(--border-muted)]"
          style={{ gridTemplateColumns: 'repeat(4, 1fr)' }}
        >
          <div className="flex flex-col items-center text-center">
            <Monitor className="w-4 h-4 text-[var(--text-tertiary)]" style={{ marginBottom: '4px' }} />
            <span className="text-xs text-[var(--text-tertiary)] uppercase">
              {tDynamic('encoder.resolution', { defaultValue: 'Resolution' })}
            </span>
            <span className="text-sm font-medium text-[var(--text-primary)]">{resolution}</span>
          </div>
          <div className="flex flex-col items-center text-center">
            <Gauge className="w-4 h-4 text-[var(--text-tertiary)]" style={{ marginBottom: '4px' }} />
            <span className="text-xs text-[var(--text-tertiary)] uppercase">
              {tDynamic('encoder.bitrate', { defaultValue: 'Bitrate' })}
            </span>
            <span className="text-sm font-medium text-[var(--text-primary)]">{bitrate}</span>
          </div>
          <div className="flex flex-col items-center text-center">
            <Film className="w-4 h-4 text-[var(--text-tertiary)]" style={{ marginBottom: '4px' }} />
            <span className="text-xs text-[var(--text-tertiary)] uppercase">
              {tDynamic('encoder.fps', { defaultValue: 'FPS' })}
            </span>
            <span className="text-sm font-medium text-[var(--text-primary)]">{fps}</span>
          </div>
          <div className="flex flex-col items-center text-center">
            <Settings2 className="w-4 h-4 text-[var(--text-tertiary)]" style={{ marginBottom: '4px' }} />
            <span className="text-xs text-[var(--text-tertiary)] uppercase">
              {tDynamic('encoder.preset', { defaultValue: 'Preset' })}
            </span>
            <span className="text-sm font-medium text-[var(--text-primary)]">{preset}</span>
          </div>
        </div>

        {/* Footer: Audio + Actions */}
        <div className="flex items-center justify-between" style={{ marginTop: '12px' }}>
          <div className="flex items-center text-sm" style={{ gap: '8px' }}>
            <Volume2 className="w-4 h-4 text-[var(--secondary)]" />
            <span className="text-[var(--text-secondary)]">
              {tDynamic('encoder.audio', { defaultValue: 'Audio' })}:
            </span>
            <span className="text-[var(--text-primary)]">{audioSummary}</span>
            {group.video.profile && (
              <>
                <span className="text-[var(--text-tertiary)]">•</span>
                <span className="text-[var(--text-secondary)]">
                  {tDynamic('encoder.profile', { defaultValue: 'Profile' })}:
                </span>
                <span className="text-[var(--text-primary)]">{profile}</span>
              </>
            )}
          </div>
          <div className="flex items-center" style={{ gap: '4px' }}>
            {!isDefaultGroup && onEdit && (
              <>
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={onEdit}
                  aria-label={tDynamic('encoder.edit', { defaultValue: 'Edit' })}
                >
                  <Pencil className="w-4 h-4" />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={onDuplicate}
                  aria-label={tDynamic('encoder.duplicate', { defaultValue: 'Duplicate' })}
                >
                  <Copy className="w-4 h-4" />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={onRemove}
                  aria-label={tDynamic('encoder.remove', { defaultValue: 'Remove' })}
                >
                  <Trash2 className="w-4 h-4" />
                </Button>
              </>
            )}
            {isDefaultGroup && (
              <span className="text-xs text-[var(--text-tertiary)] italic px-2">
                {tDynamic('encoder.defaultPassthrough', { defaultValue: 'Default RTMP relay - cannot be edited or deleted' })}
              </span>
            )}
          </div>
        </div>
      </CardBody>
    </Card>
  );
}
