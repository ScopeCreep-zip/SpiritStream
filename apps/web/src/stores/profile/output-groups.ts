import type { StateCreator } from 'zustand';
import { logger } from '@/lib/logger';
import type { OutputGroup, StreamTarget } from '@spiritstream/types';
import type { ProfileState } from './types';

type OutputGroupsSlice = Pick<
  ProfileState,
  | 'addOutputGroup'
  | 'updateOutputGroup'
  | 'removeOutputGroup'
  | 'addStreamTarget'
  | 'updateStreamTarget'
  | 'removeStreamTarget'
  | 'moveStreamTarget'
>;

export const createOutputGroupsSlice: StateCreator<ProfileState, [], [], OutputGroupsSlice> = (
  set,
  get
) => ({
  addOutputGroup: async (group) => {
    const current = get().current;
    if (current) {
      set({
        current: {
          ...current,
          outputGroups: [...current.outputGroups, group],
        },
      });
      await get().saveProfile();
    }
  },

  updateOutputGroup: async (groupId, updates) => {
    const current = get().current;
    if (current) {
      set({
        current: {
          ...current,
          outputGroups: current.outputGroups.map((g) =>
            g.id === groupId ? { ...g, ...updates } : g
          ),
        },
      });
      await get().saveProfile();
    }
  },

  removeOutputGroup: async (groupId) => {
    const current = get().current;
    if (current) {
      const groupToDelete = current.outputGroups.find((g) => g.id === groupId);
      if (groupToDelete?.isDefault) {
        // Throw instead of silently returning — the caller used to toast
        // "Output group removed" while the group was still there.
        logger.warn('Cannot delete the default passthrough output group');
        throw Object.assign(new Error('The default passthrough group cannot be removed.'), {
          kind: 'default_group_undeletable',
        });
      }

      set({
        current: {
          ...current,
          outputGroups: current.outputGroups.filter((g) => g.id !== groupId),
        },
      });
      await get().saveProfile();
    }
  },

  addStreamTarget: async (groupId, target) => {
    const current = get().current;
    if (current) {
      set({
        current: {
          ...current,
          outputGroups: current.outputGroups.map((g) =>
            g.id === groupId ? { ...g, streamTargets: [...g.streamTargets, target] } : g
          ),
        },
      });
      await get().saveProfile();
    }
  },

  updateStreamTarget: async (groupId, targetId, updates) => {
    const current = get().current;
    if (current) {
      set({
        current: {
          ...current,
          outputGroups: current.outputGroups.map((g) =>
            g.id === groupId
              ? {
                  ...g,
                  streamTargets: g.streamTargets.map((t) =>
                    t.id === targetId ? { ...t, ...updates } : t
                  ),
                }
              : g
          ),
        },
      });
      await get().saveProfile();
    }
  },

  removeStreamTarget: async (groupId, targetId) => {
    const current = get().current;
    if (current) {
      set({
        current: {
          ...current,
          outputGroups: current.outputGroups.map((g) =>
            g.id === groupId
              ? { ...g, streamTargets: g.streamTargets.filter((t) => t.id !== targetId) }
              : g
          ),
        },
      });
      await get().saveProfile();
    }
  },

  moveStreamTarget: async (fromGroupId, toGroupId, targetId) => {
    const current = get().current;
    if (!current || fromGroupId === toGroupId) return;

    const sourceGroup = current.outputGroups.find((g: OutputGroup) => g.id === fromGroupId);
    const target = sourceGroup?.streamTargets.find((t: StreamTarget) => t.id === targetId);
    if (!target) return;

    set({
      current: {
        ...current,
        outputGroups: current.outputGroups.map((g: OutputGroup) => {
          if (g.id === fromGroupId) {
            return {
              ...g,
              streamTargets: g.streamTargets.filter((t: StreamTarget) => t.id !== targetId),
            };
          }
          if (g.id === toGroupId) {
            return { ...g, streamTargets: [...g.streamTargets, target] };
          }
          return g;
        }),
      },
    });
    await get().saveProfile();
  },
});
