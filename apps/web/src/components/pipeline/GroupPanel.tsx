import React from 'react';
import { useTranslation } from 'react-i18next';
import { Pencil, Plus, Trash2, Copy } from 'lucide-react';
import { Card, CardBody, CardHeader, CardTitle } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { OutputRow, type OutputRowStatus } from './OutputRow';
import { cn } from '@/lib/cn';
import type { OutputGroup, StreamTarget } from '@spiritstream/types';

interface GroupPanelProps {
  group: OutputGroup;
  groupStatus: OutputRowStatus;
  enabledTargets: ReadonlySet<string>;
  onEditEncoder: () => void;
  onEditGroup: () => void;
  onDuplicateGroup: () => void;
  onRemoveGroup: () => void;
  onAddTarget: () => void;
  onEditTarget: (target: StreamTarget) => void;
  onRemoveTarget: (target: StreamTarget) => void;
  onToggleTargetEnabled: (target: StreamTarget) => void;
}

/**
 * Renders one output group as the focused panel under the group tab strip.
 * Tabpanel association is wired via aria-labelledby pointing back at the
 * GroupTabs tab id.
 */
export function GroupPanel({
  group,
  groupStatus,
  enabledTargets,
  onEditEncoder,
  onEditGroup,
  onDuplicateGroup,
  onRemoveGroup,
  onAddTarget,
  onEditTarget,
  onRemoveTarget,
  onToggleTargetEnabled,
}: GroupPanelProps): React.ReactElement {
  const { t } = useTranslation();
  const targets = group.streamTargets;

  return (
    <section
      role="tabpanel"
      id={`group-panel-${group.id}`}
      aria-labelledby={`group-tab-${group.id}`}
      tabIndex={0}
      className="focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default rounded-md"
    >
      <Card>
        <CardHeader>
          <CardTitle className="text-sm">
            {t('pipeline.group.encoderSummary', {
              defaultValue: '{{codec}} · {{w}}×{{h}}@{{fps}} · {{bitrate}}',
              codec: group.video.codec,
              w: group.video.width,
              h: group.video.height,
              fps: group.video.fps,
              bitrate: group.video.bitrate,
            })}
          </CardTitle>
          <div className="flex items-center gap-1">
            <Button variant="ghost" size="sm" onClick={onEditEncoder}>
              <Pencil className="w-4 h-4" aria-hidden="true" />
              {t('pipeline.group.editEncoder', { defaultValue: 'Edit encoder' })}
            </Button>
            <IconButton
              icon={<Pencil className="w-4 h-4" />}
              label={t('pipeline.group.edit', { defaultValue: 'Edit group' })}
              onClick={onEditGroup}
            />
            <IconButton
              icon={<Copy className="w-4 h-4" />}
              label={t('pipeline.group.duplicate', { defaultValue: 'Duplicate group' })}
              onClick={onDuplicateGroup}
            />
            <IconButton
              icon={<Trash2 className="w-4 h-4" />}
              label={t('pipeline.group.remove', { defaultValue: 'Remove group' })}
              onClick={onRemoveGroup}
              danger
            />
          </div>
        </CardHeader>
        <CardBody className="p-4 flex flex-col gap-3">
          <p className="text-xs text-text-tertiary">
            {group.audio.codec} · {group.audio.bitrate} · {group.audio.sampleRate / 1000}kHz · {group.audio.channels}ch
          </p>

          {targets.length > 0 ? (
            <ul className="flex flex-col gap-2">
              {targets.map((target) => (
                <OutputRow
                  key={target.id}
                  target={target}
                  status={enabledTargets.has(target.id) ? groupStatus : 'offline'}
                  enabled={enabledTargets.has(target.id)}
                  onToggleEnabled={() => onToggleTargetEnabled(target)}
                  onEdit={() => onEditTarget(target)}
                  onRemove={() => onRemoveTarget(target)}
                />
              ))}
            </ul>
          ) : (
            <p className="text-sm text-text-secondary text-center py-4">
              {t('pipeline.group.noTargets', { defaultValue: 'No targets yet — add one to start.' })}
            </p>
          )}

          <Button variant="outline" size="sm" onClick={onAddTarget} className="self-start">
            <Plus className="w-4 h-4" aria-hidden="true" />
            {t('pipeline.group.addTarget', { defaultValue: 'Add service' })}
          </Button>
        </CardBody>
      </Card>
    </section>
  );
}

interface IconButtonProps {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
  danger?: boolean;
}

function IconButton({ icon, label, onClick, danger }: IconButtonProps): React.ReactElement {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={label}
      className={cn(
        'inline-flex items-center justify-center w-8 h-8 rounded-md text-text-tertiary',
        'hover:bg-bg-hover hover:text-text-primary',
        danger && 'hover:bg-error-subtle hover:text-error-text',
        'focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default',
        'transition-colors',
      )}
    >
      {icon}
    </button>
  );
}
