import fs from "fs";
import path from "path";
import { app } from "electron";

export class Logger {
  private static instance: Logger;
  private logDir: string;

  private constructor(userDataPath: string) {
    this.logDir = path.join(userDataPath, "logs");
    this.setupLogDirectory();
  }

  public static async getInstance(): Promise<Logger> {
    if (!Logger.instance) {
      await app.whenReady();
      const userDataPath = app.getPath("userData");
      Logger.instance = new Logger(userDataPath);
    }
    return Logger.instance;
  }

  private setupLogDirectory() {
    if (!fs.existsSync(this.logDir)) {
      fs.mkdirSync(this.logDir, { recursive: true });
    }
  }

  private getTimestamp(): string {
    return new Date().toISOString();
  }

  private getLogFilePath(logFile: string = "app.log"): string {
    return path.join(this.logDir, logFile);
  }

  private writeLog(level: string, message: string, logFile: string = "app.log") {
    const timestamp = this.getTimestamp();
    const logEntry = `[${timestamp}] [${level.toUpperCase()}] ${message}\n`;
    fs.appendFileSync(this.getLogFilePath(logFile), logEntry);
  }

  public debug(message: string, logFile?: string) {
    this.writeLog("DEBUG", message, logFile);
  }

  public info(message: string, logFile?: string) {
    this.writeLog("INFO", message, logFile);
  }

  public warn(message: string, logFile?: string) {
    this.writeLog("WARN", message, logFile);
  }

  public error(message: string, logFile?: string) {
    this.writeLog("ERROR", message, logFile);
  }
}