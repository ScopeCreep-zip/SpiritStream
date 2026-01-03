import { Pencil, Copy, Trash2, Video, Volume2, Box } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/cn';
import { Card, CardHeader, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { StreamStatus } from '@/components/ui/StreamStatus';
import type { OutputGroup } from '@/types/profile';
import { formatResolution } from '@/types/profile';
import type { StreamStatusType } from '@/types/stream';

export interface OutputGroupCardProps {
  group: OutputGroup;
  index: number;
  encoders: { video: string[]; audio: string[] };
  status: StreamStatusType;
  onUpdate: (updates: Partial<OutputGroup>) => void;
  onRemove: () => void;
  onDuplicate?: () => void;
  onEdit?: () => void;
  className?: string;
}

export function OutputGroupCard({
  group,
  index,
  status,
  onRemove,
  onDuplicate,
  onEdit,
  className,
}: OutputGroupCardProps) {
  const { t } = useTranslation();

  // Display summary info from nested settings
  const videoSummary = `${formatResolution(group.video)} • ${group.video.bitrate} • ${group.video.codec}`;
  const audioSummary = `${group.audio.codec} • ${group.audio.bitrate} • ${group.audio.channels}ch`;
  const containerSummary = group.container.format.toUpperCase();

  return (
    <Card className={cn('transition-all duration-150', className)}>
      <CardHeader>
        <div className="flex items-center" style={{ gap: '12px' }}>
          <h3 className={cn('font-semibold text-[var(--text-primary)]')}>
            {group.name || t('outputs.defaultGroupName', { number: index + 1 })}
          </h3>
          <StreamStatus status={status} />
        </div>
        <div className="flex items-center" style={{ gap: '8px' }}>
          {onEdit && (
            <Button variant="ghost" size="icon" onClick={onEdit} aria-label={t('outputs.editGroup')}>
              <Pencil className="w-4 h-4" />
            </Button>
          )}
          {onDuplicate && (
            <Button variant="ghost" size="icon" onClick={onDuplicate} aria-label={t('outputs.duplicateGroup')}>
              <Copy className="w-4 h-4" />
            </Button>
          )}
          <Button variant="ghost" size="icon" onClick={onRemove} aria-label={t('outputs.removeGroup')}>
            <Trash2 className="w-4 h-4" />
          </Button>
        </div>
      </CardHeader>
      <CardBody>
        {/* Read-only display of nested settings - use Edit modal to change */}
        <div className="flex flex-col" style={{ gap: '12px' }}>
          {/* Video Settings Summary */}
          <div className="flex items-center text-sm" style={{ gap: '8px' }}>
            <Video className="w-4 h-4 text-[var(--primary)]" />
            <span className="text-[var(--text-secondary)]">{t('outputs.video')}:</span>
            <span className="text-[var(--text-primary)]">{videoSummary}</span>
          </div>

          {/* Audio Settings Summary */}
          <div className="flex items-center text-sm" style={{ gap: '8px' }}>
            <Volume2 className="w-4 h-4 text-[var(--secondary)]" />
            <span className="text-[var(--text-secondary)]">{t('outputs.audio')}:</span>
            <span className="text-[var(--text-primary)]">{audioSummary}</span>
          </div>

          {/* Container Settings Summary */}
          <div className="flex items-center text-sm" style={{ gap: '8px' }}>
            <Box className="w-4 h-4 text-[var(--accent)]" />
            <span className="text-[var(--text-secondary)]">{t('outputs.container')}:</span>
            <span className="text-[var(--text-primary)]">{containerSummary}</span>
          </div>

          {/* Stream Targets Count */}
          {group.streamTargets.length > 0 && (
            <div className="text-xs text-[var(--text-tertiary)] pt-2 border-t border-[var(--border-muted)]">
              {t('outputs.targetsCount', { count: group.streamTargets.length })}
            </div>
          )}
        </div>
      </CardBody>
    </Card>
  );
}
