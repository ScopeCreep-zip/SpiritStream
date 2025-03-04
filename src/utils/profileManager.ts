import fs from "fs";
import path from "path";
import crypto from "crypto";

const PROFILE_DIR = path.join(__dirname, "profiles");
const STATE_FILE = path.join(PROFILE_DIR, "profileState.json");
const ALGORITHM = "aes-256-cbc";
const SALT = "magillastream"; // Can be unique per profile later

if (!fs.existsSync(PROFILE_DIR)) {
  fs.mkdirSync(PROFILE_DIR, { recursive: true });
}

// 🔹 Generate a secure encryption key from a password
const deriveKey = (password: string): Buffer => {
  return crypto.pbkdf2Sync(password, SALT, 100000, 32, "sha256");
};

// 🔹 Encode an object to base64 (for unencrypted profiles)
const encodeBase64 = (data: any): string => Buffer.from(JSON.stringify(data)).toString("base64");
const decodeBase64 = (data: string): any => JSON.parse(Buffer.from(data, "base64").toString("utf-8"));

// 🔹 Encrypt a profile object
const encrypt = (data: string, password: string): string => {
  const key = deriveKey(password);
  const iv = crypto.randomBytes(16);
  const cipher = crypto.createCipheriv(ALGORITHM, key, iv);
  const encrypted = Buffer.concat([cipher.update(data), cipher.final()]);
  return iv.toString("hex") + ":" + encrypted.toString("hex");
};

// 🔹 Decrypt a profile object
const decrypt = (data: string, password: string): string | null => {
  try {
    const key = deriveKey(password);
    const [ivHex, encryptedHex] = data.split(":");
    const iv = Buffer.from(ivHex, "hex");
    const encrypted = Buffer.from(encryptedHex, "hex");
    const decipher = crypto.createDecipheriv(ALGORITHM, key, iv);
    return Buffer.concat([decipher.update(encrypted), decipher.final()]).toString();
  } catch (error) {
    return null; // Incorrect password or corrupted file
  }
};

// 🔹 Save a profile (encrypted or base64 encoded)
export const saveProfile = (profile: any, password?: string) => {
  try {
    const filePath = path.join(PROFILE_DIR, `${profile.id}.json.enc`);
    let dataToSave = password ? encrypt(JSON.stringify(profile), password) : encodeBase64(profile);
    fs.writeFileSync(filePath, dataToSave, "utf-8");
    console.log(`Profile ${profile.name} saved.`);
  } catch (error) {
    console.error("Error saving profile:", error);
  }
};

// 🔹 Load a profile (requires password if encrypted)
export const loadProfile = (profileId: string, password?: string): any | null => {
  try {
    const filePath = path.join(PROFILE_DIR, `${profileId}.json.enc`);
    if (!fs.existsSync(filePath)) return null;

    const rawData = fs.readFileSync(filePath, "utf-8");
    return password ? JSON.parse(decrypt(rawData, password) || "null") : decodeBase64(rawData);
  } catch (error) {
    console.error("Error loading profile:", error);
    return null;
  }
};

// 🔹 List available profiles (without decrypting them)
export const listProfiles = (): { id: string; name: string; encrypted: boolean }[] => {
  return fs.readdirSync(PROFILE_DIR)
    .filter(file => file.endsWith(".json.enc"))
    .map(file => {
      const id = file.replace(".json.enc", "");
      return { id, name: `Profile ${id}`, encrypted: true }; // Name must be decrypted to be accurate
    });
};

// 🔹 Remove encryption (requires password)
export const removeEncryption = (profileId: string, password: string) => {
  const profile = loadProfile(profileId, password);
  if (profile) {
    saveProfile(profile); // Save as base64 instead of encrypted
    console.log(`Encryption removed from profile ${profile.name}.`);
  } else {
    console.error("Incorrect password or profile does not exist.");
  }
};

// 🔹 Enable encryption on an existing profile
export const enableEncryption = (profileId: string, password: string) => {
  const profile = loadProfile(profileId);
  if (profile) {
    saveProfile(profile, password); // Save encrypted
    console.log(`Profile ${profile.name} is now encrypted.`);
  } else {
    console.error("Profile does not exist.");
  }
};

// 🔹 Change profile password
export const changeProfilePassword = (profileId: string, oldPassword: string, newPassword: string) => {
  const profile = loadProfile(profileId, oldPassword);
  if (profile) {
    saveProfile(profile, newPassword);
    console.log(`Password changed for profile ${profile.name}.`);
  } else {
    console.error("Incorrect password.");
  }
};

// 🔹 Delete a profile (with confirmation)
export const deleteProfile = (profileId: string) => {
  const filePath = path.join(PROFILE_DIR, `${profileId}.json.enc`);
  if (fs.existsSync(filePath)) {
    fs.unlinkSync(filePath);
    console.log(`Profile ${profileId} deleted.`);
  } else {
    console.log("Profile not found.");
  }
};

// 🔹 Store the last used profile and unsaved changes
export const saveProfileState = (profileId: string, unsavedChanges: any) => {
  const state = { lastUsedProfile: profileId, unsavedChanges };
  fs.writeFileSync(STATE_FILE, JSON.stringify(state), "utf-8");
};

// 🔹 Load the last used profile and unsaved changes
export const loadProfileState = (): { lastUsedProfile: string | null, unsavedChanges: any } => {
  if (!fs.existsSync(STATE_FILE)) return { lastUsedProfile: null, unsavedChanges: null };
  return JSON.parse(fs.readFileSync(STATE_FILE, "utf-8"));
};
