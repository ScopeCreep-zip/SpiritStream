import React, { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { GroupTabs } from './GroupTabs';
import { GroupPanel } from './GroupPanel';
import type { OutputRowStatus } from './OutputRow';
import { useProfileStore } from '@/stores/profileStore';
import { useStreamStore } from '@/stores/streamStore';
import { toast } from '@/hooks/useToast';
import { logger } from '@/lib/logger';
import { createDefaultOutputGroup } from '@/lib/profile-helpers';
import type { Profile, OutputGroup, StreamTarget } from '@spiritstream/types';

interface PipelineColumnProps {
  profile: Profile | null;
  activeGroupId: string | null;
  onSelectGroup: (id: string) => void;
  /** [+ Add service] — opens the AppDrawer pinned to the given group. */
  onAddTargetForGroup: (group: OutputGroup) => void;
  /** Row edit — opens TargetModal in edit mode with the (group, target) pair. */
  onEditTarget: (group: OutputGroup, target: StreamTarget) => void;
  /** Group edit / encoder edit — opens OutputGroupModal in edit mode. */
  onEditGroup: (group: OutputGroup) => void;
}

/**
 * Middle column — output groups tab strip + the active group's panel.
 * UI dispatch only: every mutation calls into the existing profileStore /
 * streamStore actions, which wrap the existing `api.*` methods. Validation,
 * encryption, FFmpeg orchestration all remain in crates/core.
 */
export function PipelineColumn({
  profile,
  activeGroupId,
  onSelectGroup,
  onAddTargetForGroup,
  onEditTarget,
  onEditGroup,
}: PipelineColumnProps): React.ReactElement {
  const { t } = useTranslation();
  const addOutputGroup = useProfileStore((s) => s.addOutputGroup);
  const removeOutputGroup = useProfileStore((s) => s.removeOutputGroup);
  const removeStreamTarget = useProfileStore((s) => s.removeStreamTarget);
  const setTargetEnabled = useStreamStore((s) => s.setTargetEnabled);
  const toggleTargetLive = useStreamStore((s) => s.toggleTargetLive);
  const setGroupEnabled = useStreamStore((s) => s.setGroupEnabled);
  const startGroup = useStreamStore((s) => s.startGroup);
  const stopGroup = useStreamStore((s) => s.stopGroup);
  const enabledTargets = useStreamStore((s) => s.enabledTargets);
  const enabledGroups = useStreamStore((s) => s.enabledGroups);
  const globalStatus = useStreamStore((s) => s.globalStatus);
  const activeGroups = useStreamStore((s) => s.activeGroups);

  const handleAddGroup = useCallback(async () => {
    if (!profile) return;
    try {
      const next = createDefaultOutputGroup();
      await addOutputGroup(next);
      onSelectGroup(next.id);
      toast.success(t('toast.groupAdded', { defaultValue: 'New output group added' }));
    } catch (err) {
      logger.error('[pipeline] add group failed', err);
      toast.error(
        t('toast.groupAddFailed', {
          defaultValue: 'Failed to add group: {{error}}',
          error: err instanceof Error ? err.message : String(err),
        })
      );
    }
  }, [profile, addOutputGroup, onSelectGroup, t]);

  const handleDuplicateGroup = useCallback(
    async (group: OutputGroup) => {
      try {
        const copy: OutputGroup = {
          ...group,
          id: crypto.randomUUID(),
          name: `${group.name} (Copy)`,
          streamTargets: group.streamTargets.map((t) => ({
            ...t,
            id: crypto.randomUUID(),
          })),
        };
        await addOutputGroup(copy);
        onSelectGroup(copy.id);
        toast.success(
          t('toast.groupDuplicated', {
            defaultValue: 'Duplicated {{name}}',
            name: group.name,
          })
        );
      } catch (err) {
        logger.error('[pipeline] duplicate group failed', err);
        toast.error(
          t('toast.groupDuplicateFailed', {
            defaultValue: 'Failed to duplicate group: {{error}}',
            error: err instanceof Error ? err.message : String(err),
          })
        );
      }
    },
    [addOutputGroup, onSelectGroup, t]
  );

  const handleRemoveGroup = useCallback(
    async (groupId: string) => {
      try {
        await removeOutputGroup(groupId);
        toast.success(t('toast.groupRemoved', { defaultValue: 'Output group removed' }));
      } catch (err) {
        logger.error('[pipeline] remove group failed', err);
        toast.error(
          t('toast.groupRemoveFailed', {
            defaultValue: 'Failed to remove group: {{error}}',
            error: err instanceof Error ? err.message : String(err),
          })
        );
      }
    },
    [removeOutputGroup, t]
  );

  const handleRemoveTarget = useCallback(
    async (groupId: string, target: StreamTarget) => {
      try {
        await removeStreamTarget(groupId, target.id);
        toast.success(
          t('toast.targetRemoved', {
            defaultValue: 'Removed {{name}}',
            name: target.name,
          })
        );
      } catch (err) {
        logger.error('[pipeline] remove target failed', err);
        toast.error(
          t('toast.targetRemoveFailed', {
            defaultValue: 'Failed to remove target: {{error}}',
            error: err instanceof Error ? err.message : String(err),
          })
        );
      }
    },
    [removeStreamTarget, t]
  );

  const handleToggleTarget = useCallback(
    async (group: OutputGroup, target: StreamTarget) => {
      const nextEnabled = !enabledTargets.has(target.id);

      // Pre-stream: only record the intended enable/disable in the UI set.
      // The backend reads `disabled_targets` when the group is started.
      if (!activeGroups.has(group.id)) {
        setTargetEnabled(target.id, nextEnabled);
        return;
      }

      // Live: start/stop this single target on the already-running group, so
      // a creator can drop one destination without taking the rest offline.
      if (!profile) return;
      const incomingUrl = `rtmp://${profile.input.bindAddress}:${profile.input.port}/${profile.input.application}`;
      try {
        await toggleTargetLive(target.id, nextEnabled, group, incomingUrl);
        toast.success(
          nextEnabled
            ? t('toast.targetStarted', { defaultValue: 'Started {{name}}', name: target.name })
            : t('toast.targetStopped', { defaultValue: 'Stopped {{name}}', name: target.name })
        );
      } catch (err) {
        logger.error('[pipeline] toggle target live failed', err);
        toast.error(
          t('toast.targetToggleFailed', {
            defaultValue: 'Failed to toggle {{name}}: {{error}}',
            name: target.name,
            error: err instanceof Error ? err.message : String(err),
          })
        );
      }
    },
    [enabledTargets, activeGroups, profile, setTargetEnabled, toggleTargetLive, t]
  );

  const handleStartGroup = useCallback(
    async (group: OutputGroup) => {
      if (!profile) return;
      const incomingUrl = `rtmp://${profile.input.bindAddress}:${profile.input.port}/${profile.input.application}`;
      try {
        await startGroup(group, incomingUrl);
        toast.success(
          t('toast.groupStarted', {
            defaultValue: 'Streaming {{name}}',
            name: group.name,
          })
        );
      } catch (err) {
        logger.error('[pipeline] start group failed', err);
        toast.error(
          t('toast.groupStartFailed', {
            defaultValue: 'Failed to start {{name}}: {{error}}',
            name: group.name,
            error: err instanceof Error ? err.message : String(err),
          })
        );
      }
    },
    [profile, startGroup, t]
  );

  const handleStopGroup = useCallback(
    async (group: OutputGroup) => {
      try {
        await stopGroup(group.id);
        toast.success(
          t('toast.groupStopped', {
            defaultValue: 'Stopped {{name}}',
            name: group.name,
          })
        );
      } catch (err) {
        logger.error('[pipeline] stop group failed', err);
        toast.error(
          t('toast.groupStopFailed', {
            defaultValue: 'Failed to stop {{name}}: {{error}}',
            name: group.name,
            error: err instanceof Error ? err.message : String(err),
          })
        );
      }
    },
    [stopGroup, t]
  );

  const handleToggleGroupEnabled = useCallback(
    (group: OutputGroup) => {
      // `enabledGroups` empty == every group is implicitly enabled (the
      // first-time-startup case in `startAllGroups`). Toggling off must
      // materialise the full set first, otherwise unchecking one group
      // would flip every other group out of the set too.
      if (enabledGroups.size === 0 && profile) {
        for (const g of profile.outputGroups) {
          if (g.id !== group.id) {
            setGroupEnabled(g.id, true);
          }
        }
        setGroupEnabled(group.id, false);
        return;
      }
      setGroupEnabled(group.id, !enabledGroups.has(group.id));
    },
    [enabledGroups, profile, setGroupEnabled]
  );

  if (!profile) {
    return (
      <div className="flex h-full items-center justify-center text-center text-text-tertiary text-sm p-8">
        <p>{t('pipeline.noProfile', { defaultValue: 'Load a profile to see its outputs.' })}</p>
      </div>
    );
  }

  const groups: ReadonlyArray<OutputGroup> = profile.outputGroups;
  const activeGroup = groups.find((g) => g.id === activeGroupId) ?? groups[0] ?? null;

  return (
    <div className="flex flex-col gap-4">
      <GroupTabs
        groups={groups}
        activeGroupId={activeGroup?.id ?? null}
        onSelectGroup={onSelectGroup}
        onAddGroup={handleAddGroup}
      />

      {activeGroup ? (
        <GroupPanel
          group={activeGroup}
          groupStatus={resolveGroupStatus(globalStatus, activeGroups.has(activeGroup.id))}
          enabledTargets={enabledTargets}
          isStreaming={activeGroups.has(activeGroup.id)}
          // `enabledGroups` empty == every group is implicitly enabled
          // (first-time startup before the user has toggled anything).
          isEnabled={enabledGroups.size === 0 || enabledGroups.has(activeGroup.id)}
          onEditEncoder={() => onEditGroup(activeGroup)}
          onEditGroup={() => onEditGroup(activeGroup)}
          onDuplicateGroup={() => handleDuplicateGroup(activeGroup)}
          onRemoveGroup={() => handleRemoveGroup(activeGroup.id)}
          onAddTarget={() => onAddTargetForGroup(activeGroup)}
          onEditTarget={(target) => onEditTarget(activeGroup, target)}
          onRemoveTarget={(target) => handleRemoveTarget(activeGroup.id, target)}
          onToggleTargetEnabled={(target) => handleToggleTarget(activeGroup, target)}
          onStartGroup={() => handleStartGroup(activeGroup)}
          onStopGroup={() => handleStopGroup(activeGroup)}
          onToggleGroupEnabled={() => handleToggleGroupEnabled(activeGroup)}
        />
      ) : (
        <div className="flex flex-col items-center gap-3 py-12 text-center text-text-tertiary">
          <p className="text-sm">
            {t('pipeline.noGroups', { defaultValue: 'No output groups yet.' })}
          </p>
        </div>
      )}
    </div>
  );
}

/**
 * Map the global stream status to a row-level status for an output group.
 * Inactive groups always render "offline" regardless of global state.
 */
function resolveGroupStatus(globalStatus: string, isActive: boolean): OutputRowStatus {
  if (!isActive) return 'offline';
  if (globalStatus === 'live') return 'live';
  if (globalStatus === 'connecting' || globalStatus === 'reconnecting') return 'connecting';
  if (globalStatus === 'error') return 'error';
  return 'offline';
}
