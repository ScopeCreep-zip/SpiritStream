import React, { createContext, useState, useContext, ReactNode, useEffect } from "react";
import { saveProfiles, loadProfiles } from "../utils/profileManager"; // Use Profile Manager

// 🔹 Define types for the streaming settings
type StreamTarget = {
  url: string;
  streamKey: string;
};

type OutputGroup = {
  id: string;
  name: string;
  videoEncoder: string;
  resolution: string;
  bitrate: string;
  fps: string;
  audioCodec: string;
  audioBitrate: string;
  streamTargets: StreamTarget[];
};

type Profile = {
  id: string;
  name: string;
  incomingURL: string;
  generatePTS: boolean;
  outputGroups: OutputGroup[];
};

// 🔹 Define the shape of the context
type StreamContextType = {
  profiles: Profile[];
  currentProfile: Profile | null;
  setProfiles: React.Dispatch<React.SetStateAction<Profile[]>>;
  setCurrentProfile: React.Dispatch<React.SetStateAction<Profile | null>>;
};

// 🔹 Create the context
const StreamContext = createContext<StreamContextType | undefined>(undefined);

export const StreamProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
  const [profiles, setProfiles] = useState<Profile[]>(() => {
    try {
      return loadProfiles() || []; // Load profiles from profileManager
    } catch (error) {
      console.error("Failed to load profiles:", error);
      return [];
    }
  });

  const [currentProfile, setCurrentProfile] = useState<Profile | null>(() => {
    return profiles.length > 0 ? profiles[0] : null;
  });

  // 🔹 Save profiles whenever they change
  useEffect(() => {
    try {
      saveProfiles(profiles);
    } catch (error) {
      console.error("Failed to save profiles:", error);
    }
  }, [profiles]);

  return (
    <StreamContext.Provider value={{ profiles, currentProfile, setProfiles, setCurrentProfile }}>
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
