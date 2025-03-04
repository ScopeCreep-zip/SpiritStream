import crypto from "crypto";
import fs from "fs";
import path from "path";
import { app } from "electron";
import { logger } from "../utils/logger";

const SALT_LENGTH = 16;
const IV_LENGTH = 12;
const KEY_LENGTH = 32;
const ITERATIONS = 100000;

const keyStoragePath = path.join(app.getPath("userData"), "keyfile.enc");

// 🔹 Log the key storage path
(async () => {
  const log = await logger;
  log.info(`Key storage path: ${keyStoragePath}`);
})();

// 🔹 Derive a 256-bit AES encryption key using PBKDF2
export async function deriveKey(password: string, salt: Buffer): Promise<Buffer> {
  const log = await logger;
  log.debug("Deriving encryption key...");
  return crypto.pbkdf2Sync(password, salt, ITERATIONS, KEY_LENGTH, "sha256");
}

// 🔹 Encrypt data using AES-256-GCM
export async function encryptData(password: string, data: object): Promise<string> {
  const log = await logger;
  log.info("Encrypting data...");

  const salt = crypto.randomBytes(SALT_LENGTH);
  const iv = crypto.randomBytes(IV_LENGTH);
  const key = await deriveKey(password, salt);
  const cipher = crypto.createCipheriv("aes-256-gcm", key, iv);

  try {
    const encrypted = Buffer.concat([cipher.update(JSON.stringify(data), "utf8"), cipher.final()]);
    const authTag = cipher.getAuthTag();
    log.debug("Data encryption successful.");

    return Buffer.concat([salt, iv, authTag, encrypted]).toString("base64");
  } catch (error) {
    log.error(`Encryption failed: ${error}`);
    throw error;
  }
}

// 🔹 Decrypt AES-256-GCM encrypted data
export async function decryptData(password: string, encryptedData: string): Promise<object | null> {
  const log = await logger;
  log.info("Attempting to decrypt data...");

  try {
    const buffer = Buffer.from(encryptedData, "base64");
    const salt = buffer.slice(0, SALT_LENGTH);
    const iv = buffer.slice(SALT_LENGTH, SALT_LENGTH + IV_LENGTH);
    const authTag = buffer.slice(SALT_LENGTH + IV_LENGTH, SALT_LENGTH + IV_LENGTH + 16);
    const encrypted = buffer.slice(SALT_LENGTH + IV_LENGTH + 16);
    const key = await deriveKey(password, salt);

    const decipher = crypto.createDecipheriv("aes-256-gcm", key, iv);
    decipher.setAuthTag(authTag);
    const decrypted = Buffer.concat([decipher.update(encrypted), decipher.final()]);
    log.info("Decryption successful.");

    return JSON.parse(decrypted.toString("utf8"));
  } catch (error) {
    log.error(`Decryption failed: ${error}`);
    return null;
  }
}

// 🔹 Base64 Encoding (for profiles without passwords)
export function base64Encode(data: string): string {
  return Buffer.from(data, "utf8").toString("base64");
}

// 🔹 Base64 Decoding
export function base64Decode(encodedData: string): string {
  return Buffer.from(encodedData, "base64").toString("utf8");
}

// 🔹 Securely store an encryption key (for "Remember Me" feature)
export async function storeEncryptedKey(password: string): Promise<void> {
  const log = await logger;
  log.info("Storing encrypted key...");

  const salt = crypto.randomBytes(SALT_LENGTH);
  const key = await deriveKey(password, salt);
  const encryptedKey = await encryptData(password, { key: key.toString("hex") });

  try {
    fs.writeFileSync(keyStoragePath, encryptedKey, "utf8");
    log.info("Encrypted key successfully stored.");
  } catch (error) {
    log.error(`Failed to store encrypted key: ${error}`);
  }
}

// 🔹 Retrieve a stored encryption key (if "Remember Me" is enabled)
export async function retrieveStoredKey(password: string): Promise<string | null> {
  const log = await logger;
  log.info("Retrieving stored encryption key...");

  if (!fs.existsSync(keyStoragePath)) {
    log.warn("No stored key file found.");
    return null;
  }

  try {
    const encryptedKey = fs.readFileSync(keyStoragePath, "utf8");
    const decrypted = await decryptData(password, encryptedKey);
    const decryptedData = decrypted as { key?: string } | null;

    if (decryptedData?.key) {
      log.info("Stored key successfully retrieved.");
      return decryptedData.key;
    } else {
      log.warn("Decryption succeeded but no key found.");
      return null;
    }
  } catch (error) {
    log.error(`Failed to retrieve stored key: ${error}`);
    return null;
  }
}

// 🔹 Delete stored encryption key (for logout/security reset)
export async function deleteStoredKey(): Promise<void> {
  const log = await logger;
  log.info("Deleting stored encryption key...");

  if (fs.existsSync(keyStoragePath)) {
    try {
      fs.unlinkSync(keyStoragePath);
      log.info("Stored key successfully deleted.");
    } catch (error) {
      log.error(`Failed to delete stored key: ${error}`);
    }
  } else {
    log.warn("No stored key file found to delete.");
  }
}
