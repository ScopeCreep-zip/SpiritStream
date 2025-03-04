import crypto from "crypto";
import fs from "fs";
import path from "path";
import { app } from "electron";
import { Logger } from "../utils/logger";

const SALT_LENGTH = 16;
const IV_LENGTH = 12;
const KEY_LENGTH = 32;
const ITERATIONS = 100000;

const keyStoragePath = path.join(app.getPath("userData"), "keyfile.enc");

export class Encryption {
  private static instance: Encryption;
  private logger: Logger;

  private constructor(logger: Logger) {
    this.logger = logger;
  }

  public static getInstance(): Encryption {
    if (!Encryption.instance) {
      const logger = Logger.getInstance();
      Encryption.instance = new Encryption(logger);
    }
    return Encryption.instance;
  }

  public deriveKey(password: string, salt: Buffer): Buffer {
    this.logger.debug("Deriving encryption key...");
    return crypto.pbkdf2Sync(password, salt, ITERATIONS, KEY_LENGTH, "sha256");
  }

  public encryptData(password: string, data: object): string {
    this.logger.info("Encrypting data...");
    const salt = crypto.randomBytes(SALT_LENGTH);
    const iv = crypto.randomBytes(IV_LENGTH);
    const key = this.deriveKey(password, salt);
    const cipher = crypto.createCipheriv("aes-256-gcm", key, iv);
    try {
      const encrypted = Buffer.concat([cipher.update(JSON.stringify(data), "utf8"), cipher.final()]);
      const authTag = cipher.getAuthTag();
      this.logger.debug("Data encryption successful.");
      return Buffer.concat([salt, iv, authTag, encrypted]).toString("base64");
    } catch (error) {
      this.logger.error(`Encryption failed: ${error}`);
      throw error;
    }
  }

  public decryptData(password: string, encryptedData: string): object | null {
    this.logger.info("Attempting to decrypt data...");
    try {
      const buffer = Buffer.from(encryptedData, "base64");
      const salt = buffer.subarray(0, SALT_LENGTH);
      const iv = buffer.subarray(SALT_LENGTH, SALT_LENGTH + IV_LENGTH);
      const authTag = buffer.subarray(SALT_LENGTH + IV_LENGTH, SALT_LENGTH + IV_LENGTH + 16);
      const encrypted = buffer.subarray(SALT_LENGTH + IV_LENGTH + 16);
      const key = this.deriveKey(password, salt);
      const decipher = crypto.createDecipheriv("aes-256-gcm", key, iv);
      decipher.setAuthTag(authTag);
      const decrypted = Buffer.concat([decipher.update(encrypted), decipher.final()]);
      this.logger.info("Decryption successful.");
      return JSON.parse(decrypted.toString("utf8"));
    } catch (error) {
      this.logger.error(`Decryption failed: ${error}`);
      return null;
    }
  }

  public storeEncryptedKey(password: string): void {
    this.logger.info("Storing encrypted key...");
    const salt = crypto.randomBytes(SALT_LENGTH);
    const key = this.deriveKey(password, salt);
    const encryptedKey = this.encryptData(password, { key: key.toString("hex") });
    try {
      fs.writeFileSync(keyStoragePath, encryptedKey, "utf8");
      this.logger.info("Encrypted key successfully stored.");
    } catch (error) {
      this.logger.error(`Failed to store encrypted key: ${error}`);
    }
  }

  public retrieveStoredKey(password: string): string | null {
    this.logger.info("Retrieving stored encryption key...");
    if (!fs.existsSync(keyStoragePath)) {
      this.logger.warn("No stored key file found.");
      return null;
    }
    try {
      const encryptedKey = fs.readFileSync(keyStoragePath, "utf8");
      const decrypted = this.decryptData(password, encryptedKey);
      const decryptedData = decrypted as { key?: string } | null;
      if (decryptedData?.key) {
        this.logger.info("Stored key successfully retrieved.");
        return decryptedData.key;
      } else {
        this.logger.warn("Decryption succeeded but no key found.");
        return null;
      }
    } catch (error) {
      this.logger.error(`Failed to retrieve stored key: ${error}`);
      return null;
    }
  }

  public deleteStoredKey(): void {
    this.logger.info("Deleting stored encryption key...");
    if (fs.existsSync(keyStoragePath)) {
      try {
        fs.unlinkSync(keyStoragePath);
        this.logger.info("Stored key successfully deleted.");
      } catch (error) {
        this.logger.error(`Failed to delete stored key: ${error}`);
      }
    } else {
      this.logger.warn("No stored key file found to delete.");
    }
  }

  public base64Encode(data: string): string {
    return Buffer.from(data, "utf8").toString("base64");
  }

  public base64Decode(encodedData: string): string {
    return Buffer.from(encodedData, "base64").toString("utf8");
  }
}