import React from "react";
import { useStreamContext } from "../context/streamContext";

const App: React.FC = () => {
  const { profiles, currentProfile, setProfiles, setCurrentProfile } = useStreamContext();

  // Create a new profile
  const createProfile = () => {
    const newProfile = {
      id: crypto.randomUUID(),
      name: `Profile ${profiles.length + 1}`,
      incomingURL: "",
      generatePTS: false,
      outputGroups: [],
    };

    setProfiles([...profiles, newProfile]);
    setCurrentProfile(newProfile);
  };

  // Select a profile
  const selectProfile = (id: string) => {
    const profile = profiles.find((p) => p.id === id);
    if (profile) {
      setCurrentProfile(profile);
    }
  };

  // Delete the current profile
  const deleteProfile = () => {
    if (!currentProfile) return;
    const updatedProfiles = profiles.filter((p) => p.id !== currentProfile.id);
    setProfiles(updatedProfiles);
    setCurrentProfile(updatedProfiles.length > 0 ? updatedProfiles[0] : null);
  };

  // Save the current profile (dummy function for now)
  const saveProfile = () => {
    console.log("Saving profile:", currentProfile);
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
          disabled={!currentProfile}
        >
          Save Profile
        </button>

        <button
          onClick={deleteProfile}
          className="w-full px-4 py-2 bg-red-500 dark:bg-red-700 text-white rounded-md mb-4"
          disabled={!currentProfile}
        >
          Delete Profile
        </button>

        {/* Profile Selection */}
        {profiles.length > 0 && (
          <div>
            <label className="block text-sm font-medium mb-2">Select Profile:</label>
            <select
              onChange={(e) => selectProfile(e.target.value)}
              value={currentProfile?.id || ""}
              className="w-full p-2 border rounded-md dark:bg-gray-700 dark:text-white"
            >
              {profiles.map((profile) => (
                <option key={profile.id} value={profile.id}>
                  {profile.name}
                </option>
              ))}
            </select>
          </div>
        )}
      </aside>

      {/* Main Content (Placeholder for Output Groups) */}
      <main className="flex-1 p-6">
        <h1 className="text-3xl font-bold">MagillaStream</h1>
        {currentProfile ? (
          <p className="mt-4 text-lg">Current Profile: {currentProfile.name}</p>
        ) : (
          <p className="mt-4 text-lg text-gray-500">No profile selected</p>
        )}
      </main>
    </div>
  );
};

export default App;
