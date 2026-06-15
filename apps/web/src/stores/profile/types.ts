import type {
  Profile,
  ProfileSummary,
  ProfileSettings,
  OutputGroup,
  StreamTarget,
} from '@spiritstream/types';

export interface ProfileState {
  profiles: ProfileSummary[];
  current: Profile | null;
  loading: boolean;

  pendingPasswordProfile: string | null;
  passwordError: string | null;
  pendingUnlock: boolean;

  loadProfiles: () => Promise<void>;
  loadProfile: (name: string, password?: string) => Promise<void>;
  signOut: () => Promise<void>;
  saveProfile: (password?: string) => Promise<void>;
  deleteProfile: (name: string) => Promise<void>;
  createProfile: (name: string) => Promise<void>;
  reorderProfiles: (fromIndex: number, toIndex: number) => Promise<void>;

  clearPasswordError: () => void;
  submitPassword: (password: string) => Promise<void>;
  cancelPasswordPrompt: () => void;
  unlockProfile: (name: string) => void;

  setLoading: (loading: boolean) => void;

  selectProfile: (name: string) => Promise<void>;
  duplicateProfile: (name: string) => Promise<void>;

  updateProfile: (updates: Partial<Profile>) => Promise<void>;
  updateProfileSettings: (updates: Partial<ProfileSettings>) => Promise<void>;

  addOutputGroup: (group: OutputGroup) => Promise<void>;
  updateOutputGroup: (groupId: string, updates: Partial<OutputGroup>) => Promise<void>;
  removeOutputGroup: (groupId: string) => Promise<void>;

  addStreamTarget: (groupId: string, target: StreamTarget) => Promise<void>;
  updateStreamTarget: (
    groupId: string,
    targetId: string,
    updates: Partial<StreamTarget>
  ) => Promise<void>;
  removeStreamTarget: (groupId: string, targetId: string) => Promise<void>;
  moveStreamTarget: (fromGroupId: string, toGroupId: string, targetId: string) => Promise<void>;
}
