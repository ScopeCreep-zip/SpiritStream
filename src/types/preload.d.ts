import { ProfileDTO, OutputGroupDTO } from "../shared/interfaces";

declare global {
  interface Window {
    electronAPI: {
      getUserDataPath: () => string;

      logger: {
        info: (msg: string) => void;
        error: (msg: string) => void;
      };

      profileManager: {
        getAllProfileNames: () => Promise<{ name: string; encrypted: boolean }[]>;
        loadProfile: (profileName: string, password?: string) => Promise<ProfileDTO | null>;
        saveProfile: (profile: ProfileDTO, password?: string) => Promise<void>;
        deleteProfile: (profileName: string) => Promise<void>;
        getLastUsedProfile: () => Promise<string | null>;
        saveLastUsedProfile: (name: string) => Promise<void>;
      };

      ffmpegHandler: {
        testFFmpeg: () => Promise<void>;
        startFFmpeg: (
          inputURL: string,
          outputGroups: OutputGroupDTO[],
          testMode?: boolean
        ) => Promise<void>;
        stopFFmpeg: (groupId: string) => Promise<void>;
        stopAllFFmpeg: () => Promise<void>;
        getAvailableAudioEncoders: () => Promise<string[]>;
        getAvailableVideoEncoders: () => Promise<string[]>;
      };
    };
  }
}

export {};
