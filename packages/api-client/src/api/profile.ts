import type { Profile, ProfileSummary, RtmpInput } from '@spiritstream/types';
import {
  v1ProfilesList,
  v1ProfileSummariesProxy,
  v1ProfileShow,
  v1ProfileActivate,
  v1ProfileDeactivate,
  v1ProfileUnlock,
  v1ProfileDecrypt,
  v1ProfileLock,
  v1ProfileLockedList,
  v1ProfileSave,
  v1ProfileDelete,
  v1ProfileIsEncrypted,
  v1ProfileValidateInputProxy,
  v1ProfileOrderSetProxy,
  v1ProfileOrderGetProxy,
  v1ProfileOrderEnsureProxy,
} from '../generated';

export const profile = {
  getAll: async (): Promise<string[]> => {
    const { data } = await v1ProfilesList({ throwOnError: true });
    return data.names;
  },
  getSummaries: async (): Promise<ProfileSummary[]> => {
    const { data } = await v1ProfileSummariesProxy({ throwOnError: true });
    return data as unknown as ProfileSummary[];
  },
  load: async (name: string, password?: string, _setActive: boolean = true): Promise<Profile> => {
    const { data } = await v1ProfileShow({
      path: { name },
      query: password ? { password } : undefined,
      throwOnError: true,
    });
    return data as Profile;
  },
  /**
   * Load + set-active in one round-trip. Server emits `profile_activated`
   * with consolidated state — UI stores listen for the event instead of
   * running the old `applyProfileSettings` cascade themselves.
   */
  activate: async (name: string, password?: string): Promise<Profile> => {
    const { data } = await v1ProfileActivate({
      path: { name },
      body: { password },
      throwOnError: true,
    });
    return data as Profile;
  },
  /**
   * Sign out of the active profile. The server clears active-profile state,
   * drops the anonymizer salt from memory, disconnects chat + OBS, and emits
   * `profile_deactivated`. Returns the prior active profile name (or null).
   */
  deactivate: async (): Promise<{ deactivated: string | null }> => {
    const { data } = await v1ProfileDeactivate({ throwOnError: true });
    return { deactivated: data.deactivated ?? null };
  },
  /** Unlock an encrypted profile in the server-side session unlock set. */
  unlock: async (name: string, password: string) => {
    const { data } = await v1ProfileUnlock({
      path: { name },
      body: { password },
      throwOnError: true,
    });
    return data;
  },
  /**
   * Atomic encryption removal: load with password + re-save without it
   * in one server call. Replaces the legacy two-round-trip
   * `loadProfile(password) → saveProfile(no password)` flow.
   */
  decrypt: async (name: string, password: string) => {
    const { data } = await v1ProfileDecrypt({
      path: { name },
      body: { password },
      throwOnError: true,
    });
    return data;
  },
  /** Remove a profile from the server-side session unlock set. */
  lock: async (name: string) => {
    const { data } = await v1ProfileLock({ path: { name }, throwOnError: true });
    return data;
  },
  /** List every encrypted profile currently unlocked in the session. */
  lockedList: async () => {
    const { data } = await v1ProfileLockedList({ throwOnError: true });
    return data;
  },
  /**
   * Persist a profile and return the CANONICAL document the server
   * actually wrote (input.url recomputed, PII blocklist normalized).
   * Callers should adopt the return value — the request-side copy is
   * stale the moment the server post-processes it.
   */
  save: async (profile: Profile, password?: string): Promise<Profile> => {
    const { data } = await v1ProfileSave({
      path: { name: profile.name },
      body: { profile, password },
      throwOnError: true,
    });
    return data.profile as Profile;
  },
  delete: async (name: string): Promise<void> => {
    await v1ProfileDelete({ path: { name }, throwOnError: true });
  },
  isEncrypted: async (name: string): Promise<boolean> => {
    const { data } = await v1ProfileIsEncrypted({ path: { name }, throwOnError: true });
    return data.encrypted;
  },
  validateInput: async (profileId: string, input: RtmpInput): Promise<void> => {
    await v1ProfileValidateInputProxy({ body: { profileId, input }, throwOnError: true });
  },
  setProfileOrder: async (orderedNames: string[]): Promise<void> => {
    await v1ProfileOrderSetProxy({ body: { orderedNames }, throwOnError: true });
  },
  getOrderIndexMap: async (): Promise<Record<string, number>> => {
    const { data } = await v1ProfileOrderGetProxy({ throwOnError: true });
    return data.indices;
  },
  ensureOrderIndexes: async (): Promise<Record<string, number>> => {
    const { data } = await v1ProfileOrderEnsureProxy({ throwOnError: true });
    return data.indices;
  },
};
