import React, { createContext, useState, useContext, ReactNode, useEffect } from "react";
import { ProfileManager } from "../utils/profileManager";
import { Logger } from "../utils/logger";
import { Profile } from "../models/Profile";

const profileManager = ProfileManager.getInstance();
const logger = Logger.getInstance();

// 🔹 Define the shape of the context
type StreamContextType = {
  profileNames: string[];
  currentProfile: Profile | null;
  setCurrentProfile: (profileName: string) => void;
};

// 🔹 Create the context
const StreamContext = createContext<StreamContextType | undefined>(undefined);

export const StreamProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
  const [profileNames, setProfileNames] = useState<string[]>([]);
  const [currentProfile, setCurrentProfile] = useState<Profile | null>(null);

  // 🔹 Load profile names on mount (for dropdown selection)
  useEffect(() => {
    try {
      const profiles = profileManager.getAllProfileNames().map(({ name }) => name);
      setProfileNames(profiles);

      // Load the last used profile from localStorage
      const lastUsedProfile = profileManager.getLastUsedProfile();
      if (lastUsedProfile) {
        selectProfile(lastUsedProfile);
      }
    } catch (error) {
      logger.error(`Failed to load profile names: ${error}`);
    }
  }, []);

  // 🔹 Load a profile when selected
  const selectProfile = (profileName: string) => {
    try {
      const profile = profileManager.loadProfile(profileName);
      if (profile) {
        setCurrentProfile(profile);
      } else {
        logger.warn(`Profile ${profileName} could not be loaded.`);
      }
    } catch (error) {
      logger.error(`Failed to load profile: ${error}`);
    }
  };

  // 🔹 Update lastUsedProfile in localStorage whenever the profile changes
  useEffect(() => {
    if (currentProfile) {
      profileManager.saveLastUsedProfile(currentProfile.getName());
    }
  }, [currentProfile]);

  return (
    <StreamContext.Provider value={{ profileNames, currentProfile, setCurrentProfile: selectProfile }}>
      {children}
    </StreamContext.Provider>
  );
};

// 🔹 Hook to use the context
export const useStreamContext = () => {
  const context = useContext(StreamContext);
  if (!context) {
    throw new Error("useStreamContext must be used within a StreamProvider");
  }
  return context;
};
