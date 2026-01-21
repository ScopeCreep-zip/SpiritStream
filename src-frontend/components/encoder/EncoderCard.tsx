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

type TranslateFn = (key: string, options?: Record<string, string | number>) => string;

const ENCODER_DEFAULT_LABELS: Record<string, string> = {
  libx264: 'x264',
  libx265: 'x265',
  h264_nvenc: 'NVENC',
  hevc_nvenc: 'NVENC HEVC',
  h264_videotoolbox: 'VideoToolbox',
  hevc_videotoolbox: 'VideoToolbox HEVC',
  h264_qsv: 'QuickSync',
  hevc_qsv: 'QuickSync HEVC',
  h264_amf: 'AMF',
  hevc_amf: 'AMF HEVC',
};

const HARDWARE_ENCODERS = new Set([
  'h264_nvenc',
  'hevc_nvenc',
  'h264_videotoolbox',
  'hevc_videotoolbox',
  'h264_qsv',
  'hevc_qsv',
  'h264_amf',
  'hevc_amf',
]);

const PRESET_DEFAULT_LABELS: Record<string, string> = {
  ultrafast: 'Ultrafast',
  superfast: 'Superfast',
  veryfast: 'Very Fast',
  faster: 'Faster',
  fast: 'Fast',
  medium: 'Medium',
  slow: 'Slow',
  slower: 'Slower',
  veryslow: 'Very Slow',
  quality: 'Quality',
  balanced: 'Balanced',
  speed: 'Speed',
  performance: 'Performance',
  p1: 'P1 (Fastest)',
  p2: 'P2',
  p3: 'P3',
  p4: 'P4 (Balanced)',
  p5: 'P5',
  p6: 'P6',
  p7: 'P7 (Highest Quality)',
};

/**
 * Get a human-readable label for an encoder codec
 */
function getEncoderLabel(
  codec: string,
  t: TranslateFn
): { label: string; type: 'software' | 'hardware' | 'passthrough' } {
  if (codec === 'copy') {
    return {
      label: t('encoder.passthrough', { defaultValue: 'Passthrough' }),
      type: 'passthrough',
    };
  }
  const type = HARDWARE_ENCODERS.has(codec) ? 'hardware' : 'software';
  const defaultLabel = ENCODER_DEFAULT_LABELS[codec] || codec;
  return {
    label: t(`encoder.encoders.${codec}`, { defaultValue: defaultLabel }),
    type,
  };
}

/**
 * Get a human-readable label for an encoder preset
 */
function getPresetLabel(preset: string | undefined, t: TranslateFn): string {
  if (!preset) return t('common.notAvailable');
  const defaultLabel = PRESET_DEFAULT_LABELS[preset] || preset;
  return t(`encoder.presets.${preset}`, { defaultValue: defaultLabel });
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
  const tDynamic = t as TranslateFn;

  const isPassthrough = group.video.codec === 'copy' && group.audio.codec === 'copy';
  const isDefaultGroup = group.isDefault === true;

  const encoder = getEncoderLabel(group.video.codec, tDynamic);
  const sourceLabel = tDynamic('encoder.source', { defaultValue: 'Source' });
  const resolution = isPassthrough ? sourceLabel : `${group.video.width}x${group.video.height}`;
  const bitrate = isPassthrough ? sourceLabel : group.video.bitrate;
  const fps = isPassthrough
    ? sourceLabel
    : `${group.video.fps} ${tDynamic('encoder.fpsSuffix', { defaultValue: 'fps' })}`;
  const preset = getPresetLabel(group.video.preset, tDynamic);
  const profile = group.video.profile?.toUpperCase() || t('common.notAvailable');
  const audioSummary = isPassthrough
    ? sourceLabel
    : tDynamic('encoder.audioSummary', {
        defaultValue: '{{codec}} @ {{bitrate}}',
        codec: group.audio.codec.toUpperCase(),
        bitrate: group.audio.bitrate,
      });

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
                    ({encoder.type === 'hardware'
                      ? tDynamic('encoder.hardware', { defaultValue: 'Hardware' })
                      : tDynamic('encoder.software', { defaultValue: 'Software' })})
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
                <span className="text-[var(--text-tertiary)]">|</span>
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
