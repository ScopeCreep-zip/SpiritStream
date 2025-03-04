import React, { useEffect, useState } from "react";
import { useStreamContext } from "../context/streamContext";
import { ProfileManager } from "../utils/profileManager";
import { Profile } from "../models/Profile";

const profileManager = ProfileManager.getInstance();

const App: React.FC = () => {
  const { profileNames, currentProfile, setCurrentProfile } = useStreamContext();
  const [loadedProfile, setLoadedProfile] = useState<Profile | null>(null);

  // Create a new profile
  const createProfile = () => {
    const newProfile = new Profile(
      crypto.randomUUID(),
      `Profile ${profileNames.length + 1}`,
      "",
      false
    );

    profileManager.saveProfile(newProfile);
    setCurrentProfile(newProfile.getName());  // Store just the profile name in currentProfile
  };

  // Select a profile by name
  const selectProfile = (name: string) => {
    setCurrentProfile(name);  // Just set the profile name (string)
  };

  // Load the selected profile into state
  useEffect(() => {
    if (currentProfile) {
      const profile = profileManager.loadProfile(currentProfile);
      setLoadedProfile(profile);  // Load the full Profile object when currentProfile changes
    }
  }, [currentProfile]);

  // Delete the current profile
  const deleteProfile = () => {
    if (!currentProfile) return;
    profileManager.deleteProfile(currentProfile);
    setCurrentProfile(""); // Reset to empty string
    setLoadedProfile(null);
  };

  // Save the current profile
  const saveProfile = () => {
    if (!loadedProfile) return;
    profileManager.saveProfile(loadedProfile);
  };

  return (
    <div className="h-screen flex bg-gray-100 dark:bg-gray-900 text-gray-900 dark:text-white">
      {/* Sidebar for Profile Management */}
      <aside className="w-64 p-4 bg-white dark:bg-gray-800 border-r border-gray-300 dark:border-gray-700">
        <h2 className="text-xl font-semibold mb-4">Profiles</h2>

        <button
          onClick={createProfile}
          className="w-full px-4 py-2 bg-blue-500 dark:bg-blue-700 text-white rounded-md mb-2"
        >
          Create Profile
        </button>

        <button
          onClick={saveProfile}
          className="w-full px-4 py-2 bg-green-500 dark:bg-green-700 text-white rounded-md mb-2"
          disabled={!loadedProfile}
        >
          Save Profile
        </button>

        <button
          onClick={deleteProfile}
          className="w-full px-4 py-2 bg-red-500 dark:bg-red-700 text-white rounded-md mb-4"
          disabled={!loadedProfile}
        >
          Delete Profile
        </button>

        {/* Profile Selection */}
        {profileNames.length > 0 && (
          <div>
            <label className="block text-sm font-medium mb-2">Select Profile:</label>
            <select
              onChange={(e) => selectProfile(e.target.value)}
              value={currentProfile || ""}
              className="w-full p-2 border rounded-md dark:bg-gray-700 dark:text-white"
            >
              {profileNames.map((name) => (
                <option key={name} value={name}>
                  {name}
                </option>
              ))}
            </select>
          </div>
        )}
      </aside>

      {/* Main Content (Placeholder for Output Groups) */}
      <main className="flex-1 p-6">
        <h1 className="text-3xl font-bold">MagillaStream</h1>
        {loadedProfile ? (
          <p className="mt-4 text-lg">Current Profile: {loadedProfile.getName()}</p> {/* Display full profile name */}
        ) : (
          <p className="mt-4 text-lg text-gray-500">No profile selected</p>
        )}
      </main>
    </div>
  );
};

export default App;
