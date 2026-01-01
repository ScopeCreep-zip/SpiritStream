import { Pencil, Copy, Trash2 } from 'lucide-react';
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
        <div className="flex items-center gap-3">
          <h3 className={cn('font-semibold text-[var(--text-primary)]')}>
            {group.name || `Output Group ${index + 1}`}
          </h3>
          <StreamStatus status={status} />
        </div>
        <div className="flex items-center gap-2">
          {onEdit && (
            <Button variant="ghost" size="icon" onClick={onEdit} aria-label="Edit output group">
              <Pencil className="w-4 h-4" />
            </Button>
          )}
          {onDuplicate && (
            <Button variant="ghost" size="icon" onClick={onDuplicate} aria-label="Duplicate output group">
              <Copy className="w-4 h-4" />
            </Button>
          )}
          <Button variant="ghost" size="icon" onClick={onRemove} aria-label="Remove output group">
            <Trash2 className="w-4 h-4" />
          </Button>
        </div>
      </CardHeader>
      <CardBody className={cn('space-y-4')}>
        <div className={cn('grid grid-cols-2 gap-4')}>
          <Select
            label="Video Encoder"
            value={group.videoEncoder}
            onChange={(e) => onUpdate({ videoEncoder: e.target.value })}
            options={encoders.video.map((e) => ({ value: e, label: e }))}
          />
          <Select
            label="Resolution"
            value={group.resolution}
            onChange={(e) => onUpdate({ resolution: e.target.value })}
            options={resolutionOptions}
          />
        </div>
        <div className={cn('grid grid-cols-3 gap-4')}>
          <Input
            label="Video Bitrate (kbps)"
            type="number"
            value={group.videoBitrate}
            onChange={(e) => onUpdate({ videoBitrate: parseInt(e.target.value) || 0 })}
          />
          <Input
            label="FPS"
            type="number"
            value={group.fps}
            onChange={(e) => onUpdate({ fps: parseInt(e.target.value) || 30 })}
          />
          <Input
            label="Audio Bitrate (kbps)"
            type="number"
            value={group.audioBitrate}
            onChange={(e) => onUpdate({ audioBitrate: parseInt(e.target.value) || 0 })}
          />
        </div>
        <div className={cn('grid grid-cols-2 gap-4')}>
          <Select
            label="Audio Codec"
            value={group.audioCodec}
            onChange={(e) => onUpdate({ audioCodec: e.target.value })}
            options={encoders.audio.map((e) => ({ value: e, label: e }))}
          />
          <div className="flex items-end pb-1">
            <Toggle
              label="Generate PTS timestamps"
              checked={group.generatePts}
              onChange={(checked) => onUpdate({ generatePts: checked })}
            />
          </div>
        </div>
      </CardBody>
    </Card>
  );
}
