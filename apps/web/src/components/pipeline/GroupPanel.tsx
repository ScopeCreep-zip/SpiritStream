import React from 'react';
import { useTranslation } from 'react-i18next';
import { Pencil, Plus, Trash2, Copy, Play, Square } from 'lucide-react';
import { Card, CardBody, CardHeader, CardTitle } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { OutputRow, type OutputRowStatus } from './OutputRow';
import { cn } from '@/lib/cn';
import type { ChatPlatform, ChatPlatformStatus, OutputGroup, StreamTarget } from '@spiritstream/types';

interface GroupPanelProps {
  group: OutputGroup;
  groupStatus: OutputRowStatus;
  enabledTargets: ReadonlySet<string>;
  /** Whether this group is currently streaming (member of `streamStore.activeGroups`). */
  isStreaming: boolean;
  /** Whether the group participates in `Start all streams`. */
  isEnabled: boolean;
  onEditEncoder: () => void;
  onDuplicateGroup: () => void;
  onRemoveGroup: () => void;
  onAddTarget: () => void;
  onEditTarget: (target: StreamTarget) => void;
  onRemoveTarget: (target: StreamTarget) => void;
  onToggleTargetEnabled: (target: StreamTarget) => void;
  /** Jump to a target service's chat settings (icon shown only for chat-capable services). */
  onOpenChatSettings?: (platform: ChatPlatform) => void;
  /** Resolve a platform's live chat status, or null when not set up (gates the row toggle). */
  chatConnectionFor?: (platform: ChatPlatform) => ChatPlatformStatus['status'] | null;
  /** Per-group start — wires `streamStore.startGroup`. */
  onStartGroup: () => void;
  /** Per-group stop — wires `streamStore.stopGroup`. */
  onStopGroup: () => void;
  /** Toggle whether this group is included in `Start all streams`. */
  onToggleGroupEnabled: () => void;
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
  isStreaming,
  isEnabled,
  onEditEncoder,
  onDuplicateGroup,
  onRemoveGroup,
  onAddTarget,
  onEditTarget,
  onRemoveTarget,
  onToggleTargetEnabled,
  onOpenChatSettings,
  chatConnectionFor,
  onStartGroup,
  onStopGroup,
  onToggleGroupEnabled,
}: GroupPanelProps): React.ReactElement {
  const { t } = useTranslation();
  const targets = group.streamTargets;
  // Default passthrough is RTMP-relay only — backend refuses edit/delete and
  // `OutputGroupModal` self-closes in edit mode for it. Disable the four
  // mutating actions so clicks don't silently no-op.
  const isPassthrough = group.isDefault === true;
  // Per-group start needs at least one enabled target to push to; otherwise
  // ffmpeg would spawn with zero outputs and immediately exit.
  const canStart =
    targets.length > 0 && targets.some((tgt) => enabledTargets.has(tgt.id));

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
            {isStreaming ? (
              <Button
                variant="ghost"
                size="sm"
                onClick={onStopGroup}
                aria-label={t('pipeline.group.stop', { defaultValue: 'Stop this group' })}
              >
                <Square className="w-4 h-4" aria-hidden="true" />
                {t('pipeline.group.stop', { defaultValue: 'Stop' })}
              </Button>
            ) : (
              <Button
                variant="ghost"
                size="sm"
                onClick={onStartGroup}
                disabled={!canStart}
                aria-label={t('pipeline.group.start', { defaultValue: 'Start this group' })}
                title={
                  canStart
                    ? undefined
                    : t('pipeline.group.startDisabled', {
                        defaultValue: 'Enable at least one target to start',
                      })
                }
              >
                <Play className="w-4 h-4" aria-hidden="true" />
                {t('pipeline.group.start', { defaultValue: 'Start' })}
              </Button>
            )}
            <label className="inline-flex items-center gap-1 text-xs text-text-tertiary px-2">
              <input
                type="checkbox"
                checked={isEnabled}
                onChange={onToggleGroupEnabled}
                disabled={isPassthrough}
                aria-label={t('pipeline.group.enabledInStartAll', {
                  defaultValue: 'Include in start all',
                })}
              />
              <span>
                {t('pipeline.group.enabledInStartAll', {
                  defaultValue: 'Include in start all',
                })}
              </span>
            </label>
            <Button
              variant="ghost"
              size="sm"
              onClick={onEditEncoder}
              disabled={isPassthrough}
            >
              <Pencil className="w-4 h-4" aria-hidden="true" />
              {t('pipeline.group.editEncoder', { defaultValue: 'Edit encoder' })}
            </Button>
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
              disabled={isPassthrough}
            />
          </div>
        </CardHeader>
        <CardBody className="p-4 flex flex-col gap-3">
          <p className="text-xs text-text-tertiary">
            {group.audio.codec} · {group.audio.bitrate} · {group.audio.sampleRate / 1000}kHz ·{' '}
            {group.audio.channels}ch
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
                  onOpenChatSettings={onOpenChatSettings}
                  chatConnectionFor={chatConnectionFor}
                />
              ))}
            </ul>
          ) : (
            <p className="text-sm text-text-secondary text-center py-4">
              {t('pipeline.group.noTargets', {
                defaultValue: 'No targets yet — add one to start.',
              })}
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
  disabled?: boolean;
}

function IconButton({
  icon,
  label,
  onClick,
  danger,
  disabled,
}: IconButtonProps): React.ReactElement {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={label}
      disabled={disabled}
      className={cn(
        'inline-flex items-center justify-center w-8 h-8 rounded-md text-text-tertiary',
        'hover:bg-bg-hover hover:text-text-primary',
        danger && 'hover:bg-error-subtle hover:text-error-text',
        'focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default',
        'disabled:opacity-50 disabled:pointer-events-none',
        'transition-colors'
      )}
    >
      {icon}
    </button>
  );
}
