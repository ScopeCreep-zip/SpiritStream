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

  public static async getInstance(): Promise<Encryption> {
    if (!Encryption.instance) {
      const logger = await Logger.getInstance();
      Encryption.instance = new Encryption(logger);
    }
    return Encryption.instance;
  }

  // Derive a 256-bit AES encryption key using PBKDF2
  public async deriveKey(password: string, salt: Buffer): Promise<Buffer> {
    this.logger.debug("Deriving encryption key...");
    return crypto.pbkdf2Sync(password, salt, ITERATIONS, KEY_LENGTH, "sha256");
  }

  // Encrypt data using AES-256-GCM
  public async encryptData(password: string, data: object): Promise<string> {
    this.logger.info("Encrypting data...");

    const salt = crypto.randomBytes(SALT_LENGTH);
    const iv = crypto.randomBytes(IV_LENGTH);
    const key = await this.deriveKey(password, salt);
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

  // Decrypt AES-256-GCM encrypted data
  public async decryptData(password: string, encryptedData: string): Promise<object | null> {
    this.logger.info("Attempting to decrypt data...");

    try {
      const buffer = Buffer.from(encryptedData, "base64");
      const salt = buffer.slice(0, SALT_LENGTH);
      const iv = buffer.slice(SALT_LENGTH, SALT_LENGTH + IV_LENGTH);
      const authTag = buffer.slice(SALT_LENGTH + IV_LENGTH, SALT_LENGTH + IV_LENGTH + 16);
      const encrypted = buffer.slice(SALT_LENGTH + IV_LENGTH + 16);
      const key = await this.deriveKey(password, salt);

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

  // Base64 Encoding (for profiles without passwords)
  public base64Encode(data: string): string {
    return Buffer.from(data, "utf8").toString("base64");
  }

  // Base64 Decoding
  public base64Decode(encodedData: string): string {
    return Buffer.from(encodedData, "base64").toString("utf8");
  }
}
