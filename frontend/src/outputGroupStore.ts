// outputGroupStore.ts
import { writable } from 'svelte/store';

// Define the types for stream target and output group
export interface StreamTarget {
  url: string;
  key: string;
}

export interface OutputGroup {
  name: string;
  forwardOriginal: boolean;
  streamTargets: StreamTarget[];
}

// Create a writable store for output groups
export const outputGroups = writable<OutputGroup[]>([]);

// Function to add a new output group
export function addOutputGroup() {
  const newGroup: OutputGroup = {
    name: '',
    forwardOriginal: false,
    streamTargets: [{ url: '', key: '' }]  // Default stream target
  };

  outputGroups.update(groups => [...groups, newGroup]);  // Add new group
}

// Function to add a stream target to an output group
export function addStreamTarget(groupIndex: number) {
  outputGroups.update(groups => {
    return groups.map((group, idx) => idx === groupIndex
      ? { ...group, streamTargets: [...group.streamTargets, { url: '', key: '' }] }
      : group
    );
  });
}

// Function to remove a stream target from an output group
export function removeStreamTarget(groupIndex: number, targetIndex: number) {
  outputGroups.update(groups => {
    const updatedGroups = [...groups];
    updatedGroups[groupIndex].streamTargets = updatedGroups[groupIndex].streamTargets.filter((_, index) => index !== targetIndex);
    return updatedGroups;
  });
}
