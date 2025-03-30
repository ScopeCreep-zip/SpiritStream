import fs from "fs";
import path from "path";
import { app } from "electron";
import { Profile } from "../models/Profile";
import { Encryption } from "../utils/encryption";
import { Logger } from "../utils/logger";

const PROFILE_DIR = path.join(app.getPath("userData"), "profiles");
const LAST_USED_PROFILE_KEY = "lastUsedProfile";

export class ProfileManager {
  private static instance: ProfileManager;
  private logger: Logger;
  private encryption: Encryption;

  private constructor(logger: Logger, encryption: Encryption) {
    this.logger = logger;
    this.encryption = encryption;

    // Ensure the profile directory exists
    if (!fs.existsSync(PROFILE_DIR)) {
      fs.mkdirSync(PROFILE_DIR, { recursive: true });
      this.logger.info(`Created profile directory: ${PROFILE_DIR}`);
    }
  }

  // Make getInstance() synchronous
  public static getInstance(): ProfileManager {
    if (!ProfileManager.instance) {
      const logger = Logger.getInstance();
      const encryption = Encryption.getInstance();
      ProfileManager.instance = new ProfileManager(logger, encryption);
    }
    return ProfileManager.instance;
  }

  // Get all profile names for UI dropdown
  public getAllProfileNames(): { name: string; encrypted: boolean }[] {
    if (!fs.existsSync(PROFILE_DIR)) return [];
    this.logger.info("Fetching all profile names.");

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
    this.logger.info(`Last used profile set to: ${profileName}`);
  }

  // Retrieve the last used profile name from localStorage
  public getLastUsedProfile(): string | null {
    const storedData = localStorage.getItem(LAST_USED_PROFILE_KEY);
    return storedData ? JSON.parse(storedData).lastUsedProfile : null;
  }


  public saveProfile(profile: Profile, password?: string) {
    const filePath = path.join(PROFILE_DIR, `${profile.getName()}.json`);
    let data = profile.export();

    try {
      if (password) {
        data = this.encryption.encryptData(password, JSON.parse(data));
        this.logger.info(`Profile ${profile.getName()} saved with encryption.`);
      } else {
        data = this.encryption.base64Encode(data);
        this.logger.info(`Profile ${profile.getName()} saved with base64 encoding.`);
      }

      fs.writeFileSync(filePath, data);
      this.saveLastUsedProfile(profile.getName());
    } catch (error) {
      this.logger.error(`Failed to save profile: ${error}`);
    }
  }

  public loadProfile(profileName: string, password?: string): Profile | null {
    const filePath = path.join(PROFILE_DIR, `${profileName}.json`);
    if (!fs.existsSync(filePath)) {
      this.logger.warn(`Profile ${profileName} does not exist.`);
      return null;
    }

    let data = fs.readFileSync(filePath, "utf8");
    try {
      if (password) {
        data = JSON.stringify(this.encryption.decryptData(password, data));
        this.logger.info(`Profile ${profileName} decrypted successfully.`);
      } else {
        data = this.encryption.base64Decode(data);
        this.logger.info(`Profile ${profileName} loaded using base64 decoding.`);
      }

      const profileObj = JSON.parse(data);
      return new Profile(profileObj.id, profileObj.name, profileObj.incomingURL, profileObj.generatePTS);
    } catch (error) {
      this.logger.error(`Failed to load profile: ${error}`);
      return null;
    }
  }

  // Delete a profile from disk
  public deleteProfile(profileName: string) {
    const filePath = path.join(PROFILE_DIR, `${profileName}.json`);
    if (fs.existsSync(filePath)) {
      fs.unlinkSync(filePath);
      this.logger.info(`Profile "${profileName}" deleted.`);
    } else {
      this.logger.warn(`Attempted to delete non-existent profile: ${profileName}`);
    }
  }
}
