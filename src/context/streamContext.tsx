import React, { createContext, useState, useContext, ReactNode } from "react";

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

// 🔹 Context Provider Component
export const StreamProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [currentProfile, setCurrentProfile] = useState<Profile | null>(null);

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
