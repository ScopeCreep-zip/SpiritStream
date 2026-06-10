import React from 'react';
import { useTranslation } from 'react-i18next';
import { Cpu, Pencil } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import type { OutputGroup } from '@spiritstream/types';

interface EncoderCardProps {
  /** Active output group whose encoder we're showing. */
  group: OutputGroup | null;
  /** Opens the encoder editor. Undefined when no group is selected. */
  onEdit?: () => void;
}

export function EncoderCard({ group, onEdit }: EncoderCardProps): React.ReactElement {
  const { t } = useTranslation();
  // Default passthrough is RTMP-relay only — `OutputGroupModal` refuses to
  // open in edit mode for it (see OutputGroupModal:81-87). Surface the
  // disabled affordance + the create-a-custom-group hint here instead of
  // letting the click no-op.
  const isPassthrough = group?.isDefault === true;
  const canEdit = !!group && !!onEdit && !isPassthrough;

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Cpu className="w-4 h-4 text-text-tertiary" aria-hidden="true" />
          {t('input.encoder.title', { defaultValue: 'Encoder' })}
        </CardTitle>
      </CardHeader>
      <CardBody className="p-4 flex flex-col gap-3">
        {group ? (
          <dl className="text-sm grid grid-cols-[auto_1fr] gap-x-3 gap-y-1">
            <dt className="text-text-tertiary text-xs">
              {t('input.encoder.video', { defaultValue: 'Video' })}
            </dt>
            <dd className="text-text-primary text-xs font-mono">
              {group.video.codec} · {group.video.width}×{group.video.height}@{group.video.fps} ·{' '}
              {group.video.bitrate}
            </dd>
            <dt className="text-text-tertiary text-xs">
              {t('input.encoder.audio', { defaultValue: 'Audio' })}
            </dt>
            <dd className="text-text-primary text-xs font-mono">
              {group.audio.codec} · {group.audio.bitrate} · {group.audio.sampleRate / 1000}kHz
            </dd>
          </dl>
        ) : (
          <p className="text-sm text-text-secondary">
            {t('input.encoder.noGroup', { defaultValue: 'No output group selected.' })}
          </p>
        )}
        {isPassthrough && (
          <p className="text-xs text-text-tertiary">
            {t('input.encoder.passthroughHint', {
              defaultValue:
                'Default passthrough relays your stream as-is. Add a custom output group to configure encoder settings.',
            })}
          </p>
        )}
        <Button
          variant="outline"
          size="sm"
          onClick={onEdit}
          disabled={!canEdit}
          className="self-start"
        >
          <Pencil className="w-4 h-4" aria-hidden="true" />
          {t('common.edit', { defaultValue: 'Edit' })}
        </Button>
      </CardBody>
    </Card>
  );
}
