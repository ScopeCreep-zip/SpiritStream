import { Pencil, Copy, Trash2 } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/cn';
import { Card, CardHeader, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Select } from '@/components/ui/Select';
import { Input } from '@/components/ui/Input';
import { Toggle } from '@/components/ui/Toggle';
import { StreamStatus } from '@/components/ui/StreamStatus';
import type { OutputGroup } from '@/types/profile';
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
  encoders,
  status,
  onUpdate,
  onRemove,
  onDuplicate,
  onEdit,
  className,
}: OutputGroupCardProps) {
  const { t } = useTranslation();

  const resolutionOptions = [
    { value: '1920x1080', label: '1080p (1920x1080)' },
    { value: '1280x720', label: '720p (1280x720)' },
    { value: '854x480', label: '480p (854x480)' },
    { value: '2560x1440', label: '1440p (2560x1440)' },
    { value: '3840x2160', label: '4K (3840x2160)' },
  ];

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
        <div className="flex flex-col" style={{ gap: '16px' }}>
        <div className={cn('grid grid-cols-2')} style={{ gap: '16px' }}>
          <Select
            label={t('encoder.videoEncoder')}
            value={group.videoEncoder}
            onChange={(e) => onUpdate({ videoEncoder: e.target.value })}
            options={encoders.video.map((e) => ({ value: e, label: e }))}
          />
          <Select
            label={t('encoder.resolution')}
            value={group.resolution}
            onChange={(e) => onUpdate({ resolution: e.target.value })}
            options={resolutionOptions}
          />
        </div>
        <div className={cn('grid grid-cols-3')} style={{ gap: '16px' }}>
          <Input
            label={t('encoder.videoBitrateKbps')}
            type="number"
            value={group.videoBitrate}
            onChange={(e) => onUpdate({ videoBitrate: parseInt(e.target.value) || 0 })}
          />
          <Input
            label={t('encoder.fps')}
            type="number"
            value={group.fps}
            onChange={(e) => onUpdate({ fps: parseInt(e.target.value) || 30 })}
          />
          <Input
            label={t('encoder.audioBitrateKbps')}
            type="number"
            value={group.audioBitrate}
            onChange={(e) => onUpdate({ audioBitrate: parseInt(e.target.value) || 0 })}
          />
        </div>
        <div className={cn('grid grid-cols-2')} style={{ gap: '16px' }}>
          <Select
            label={t('encoder.audioCodec')}
            value={group.audioCodec}
            onChange={(e) => onUpdate({ audioCodec: e.target.value })}
            options={encoders.audio.map((e) => ({ value: e, label: e }))}
          />
          <div className="flex items-end" style={{ paddingBottom: '4px' }}>
            <Toggle
              label={t('encoder.generatePts')}
              checked={group.generatePts}
              onChange={(checked) => onUpdate({ generatePts: checked })}
            />
          </div>
        </div>
        </div>
      </CardBody>
    </Card>
  );
}
