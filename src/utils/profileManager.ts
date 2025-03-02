import fs from "fs";
import path from "path";
import { app } from "electron";
import { encryptData, decryptData, retrieveStoredKey } from "../utils/encryption";
import { logger } from "../utils/logger";

const profilesPath = path.join(app.getPath("userData"), "profiles.json");

(async () => {
  const log = await logger;
  log.info(`Profile storage path: ${profilesPath}`);
})();

/**
 * Saves profiles securely.
 */
export async function saveProfiles(profiles: object[], password: string): Promise<void> {
  const log = await logger;
  log.info("Attempting to save profiles...");
  log.debug(`Profiles data: ${JSON.stringify(profiles, null, 2)}`);

  const encryptedData = await encryptData(password, { profiles });

  try {
    fs.writeFileSync(profilesPath, encryptedData, "utf8");
    log.info(`Profiles successfully saved at: ${profilesPath}`);
  } catch (error) {
    log.error(`Failed to save profiles: ${error}`);
  }
}

/**
 * Loads profiles securely.
 */
export async function loadProfiles(password: string): Promise<object[]> {
  const log = await logger;
  log.info("Attempting to load profiles...");

  if (!fs.existsSync(profilesPath)) {
    log.warn("No profile file found. Returning empty profile list.");
    return [];
  }

  try {
    const encryptedData = fs.readFileSync(profilesPath, "utf8");
    const decrypted = (await decryptData(password, encryptedData)) as { profiles?: object[] } | null;

    if (decrypted?.profiles) {
      log.info("Profiles successfully loaded.");
      return decrypted.profiles;
    } else {
      log.warn("Decryption succeeded but returned empty profiles.");
      return [];
    }
  } catch (error) {
    log.error(`Failed to load profiles: ${error}`);
    return [];
  }
}

/**
 * Loads profiles using a stored key (if available).
 */
export async function loadProfilesWithStoredKey(): Promise<object[]> {
  const log = await logger;
  log.info("Attempting to load profiles using stored key...");

  const storedKey = await retrieveStoredKey("dummy-password"); // Replace with real key handling
  if (!storedKey) {
    log.warn("No stored key found. Cannot auto-load profiles.");
    return [];
  }

  return loadProfiles(storedKey);
}

/**
 * Deletes the stored profiles file.
 */
export async function deleteProfiles(): Promise<void> {
  const log = await logger;
  log.info("Deleting stored profiles...");

  if (fs.existsSync(profilesPath)) {
    try {
      fs.unlinkSync(profilesPath);
      log.info("Stored profiles successfully deleted.");
    } catch (error) {
      log.error(`Failed to delete stored profiles: ${error}`);
    }
  } else {
    log.warn("No stored profile file found to delete.");
  }
}
