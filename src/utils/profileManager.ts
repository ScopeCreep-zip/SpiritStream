import fs from "fs";
import path from "path";
import { app } from "electron";
import { Profile } from "../models/Profile";
import { encryptData, decryptData, base64Encode, base64Decode } from "../utils/encryption";

const PROFILE_DIR = path.join(app.getPath("userData"), "profiles");
const LAST_USED_PROFILE_KEY = "lastUsedProfile";

// Ensure the profile directory exists
if (!fs.existsSync(PROFILE_DIR)) {
  fs.mkdirSync(PROFILE_DIR, { recursive: true });
}

export class ProfileManager {
  private static instance: ProfileManager;

  private constructor() {}

  public static getInstance(): ProfileManager {
    if (!ProfileManager.instance) {
      ProfileManager.instance = new ProfileManager();
    }
    return ProfileManager.instance;
  }

  // Get all profile names for UI dropdown
  public getAllProfileNames(): { name: string; encrypted: boolean }[] {
    if (!fs.existsSync(PROFILE_DIR)) return [];

    return fs.readdirSync(PROFILE_DIR)
      .filter(file => file.endsWith(".json"))
      .map(file => {
        const profileName = path.basename(file, ".json");
        const filePath = path.join(PROFILE_DIR, file);
        const data = fs.readFileSync(filePath, "utf8");

        // Determine if it's encrypted (Base64 or AES)
        const encrypted = /^[A-Za-z0-9+/]+={0,2}$/.test(data.trim()) || data.includes(":");
        return { name: profileName, encrypted };
      });
  }

  // Save the last used profile name to localStorage
  public saveLastUsedProfile(profileName: string) {
    localStorage.setItem(LAST_USED_PROFILE_KEY, JSON.stringify({ lastUsedProfile: profileName }));
  }

  // Retrieve the last used profile name from localStorage
  public getLastUsedProfile(): string | null {
    const storedData = localStorage.getItem(LAST_USED_PROFILE_KEY);
    return storedData ? JSON.parse(storedData).lastUsedProfile : null;
  }

  // Save a profile to disk (encrypt if password is provided)
  public saveProfile(profile: Profile, password?: string) {
    const filePath = path.join(PROFILE_DIR, `${profile.name}.json`);
    let data = JSON.stringify(profile, null, 2);

    if (password) {
      data = encryptData(data, password);
    } else {
      data = base64Encode(data);
    }

    fs.writeFileSync(filePath, data);
    this.saveLastUsedProfile(profile.name);
  }

  // Load a profile from disk (decrypt if needed)
  public loadProfile(profileName: string, password?: string): Profile | null {
    const filePath = path.join(PROFILE_DIR, `${profileName}.json`);
    if (!fs.existsSync(filePath)) return null;

    let data = fs.readFileSync(filePath, "utf8");
    try {
      if (password) {
        data = decryptData(data, password);
      } else {
        data = base64Decode(data);
      }

      // Instantiate Profile class from JSON data
      const profileObj = JSON.parse(data);
      return new Profile(profileObj.id, profileObj.name, profileObj.incomingURL, profileObj.generatePTS);
    } catch (err) {
      console.error("Error loading profile:", err);
      return null;
    }
  }

  // Delete a profile from disk
  public deleteProfile(profileName: string) {
    const filePath = path.join(PROFILE_DIR, `${profileName}.json`);
    if (fs.existsSync(filePath)) {
      fs.unlinkSync(filePath);
      console.log(`Profile "${profileName}" deleted.`);
    }
  }
}