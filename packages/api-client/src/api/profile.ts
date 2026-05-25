import type { Profile, ProfileSummary, RtmpInput } from '@spiritstream/types';
import { fetchTypedJson } from './_internal';

export const profile = {
  getAll: async () => {
    const { names } = await fetchTypedJson<{ names: string[] }>('GET', '/api/v1/profiles');
    return names;
  },
  getSummaries: () => fetchTypedJson<ProfileSummary[]>('GET', '/api/v1/profiles/summaries'),
  load: (name: string, password?: string, _setActive: boolean = true) =>
    fetchTypedJson<Profile>(
      'GET',
      `/api/v1/profiles/${encodeURIComponent(name)}`,
      password ? { password } : undefined,
    ),
  /**
   * Load + set-active in one round-trip. Server emits `profile_activated`
   * with consolidated state — UI stores listen for the event instead of
   * running the old `applyProfileSettings` cascade themselves.
   */
  activate: (name: string, password?: string) =>
    fetchTypedJson<Profile>(
      'POST',
      `/api/v1/profiles/${encodeURIComponent(name)}/activate`,
      undefined,
      { password },
    ),
  /** Unlock an encrypted profile in the server-side session unlock set. */
  unlock: (name: string, password: string) =>
    fetchTypedJson<{ name: string; unlocked: boolean }>(
      'POST',
      `/api/v1/profiles/${encodeURIComponent(name)}/unlock`,
      undefined,
      { password },
    ),
  /**
   * Atomic encryption removal: load with password + re-save without it
   * in one server call. Replaces the legacy two-round-trip
   * `loadProfile(password) → saveProfile(no password)` flow.
   */
  decrypt: (name: string, password: string) =>
    fetchTypedJson<{ name: string; decrypted: boolean }>(
      'POST',
      `/api/v1/profiles/${encodeURIComponent(name)}/decrypt`,
      undefined,
      { password },
    ),
  /** Remove a profile from the server-side session unlock set. */
  lock: (name: string) =>
    fetchTypedJson<{ name: string; locked: boolean }>(
      'POST',
      `/api/v1/profiles/${encodeURIComponent(name)}/lock`,
    ),
  /** List every encrypted profile currently unlocked in the session. */
  lockedList: () =>
    fetchTypedJson<{ unlocked: string[] }>('GET', '/api/v1/profiles/locked'),
  save: async (profile: Profile, password?: string) => {
    await fetchTypedJson<{ saved: boolean }>(
      'PUT',
      `/api/v1/profiles/${encodeURIComponent(profile.name)}`,
      undefined,
      { profile, password },
    );
  },
  delete: async (name: string) => {
    await fetchTypedJson<{ deleted: boolean }>(
      'DELETE',
      `/api/v1/profiles/${encodeURIComponent(name)}`,
    );
  },
  isEncrypted: async (name: string) => {
    const { encrypted } = await fetchTypedJson<{ encrypted: boolean }>(
      'GET',
      `/api/v1/profiles/${encodeURIComponent(name)}/encrypted`,
    );
    return encrypted;
  },
  validateInput: async (profileId: string, input: RtmpInput) => {
    await fetchTypedJson<unknown>('POST', '/api/v1/profiles/validate-input', undefined, {
      profileId,
      input,
    });
  },
  setProfileOrder: async (orderedNames: string[]) => {
    await fetchTypedJson<unknown>('PATCH', '/api/v1/profiles/order', undefined, { orderedNames });
  },
  getOrderIndexMap: () =>
    fetchTypedJson<Record<string, number>>('GET', '/api/v1/profiles/order'),
  ensureOrderIndexes: () =>
    fetchTypedJson<Record<string, number>>('POST', '/api/v1/profiles/order/ensure'),
};
