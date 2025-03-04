import fs from "fs";
import path from "path";
import { app } from "electron";

export class Logger {
  private static instance: Logger;
  private logDir: string;
  private logFile: string;

  private constructor(userDataPath: string) {
    this.logDir = path.join(userDataPath, "logs");
    this.logFile = path.join(this.logDir, "app.log");
    this.setupLogFile();
  }

  public static async getInstance(): Promise<Logger> {
    if (!Logger.instance) {
      await app.whenReady();
      const userDataPath = app.getPath("userData");
      Logger.instance = new Logger(userDataPath);
    }
    return Logger.instance;
  }

  private setupLogFile() {
    if (!fs.existsSync(this.logDir)) {
      fs.mkdirSync(this.logDir, { recursive: true });
    }
    if (!fs.existsSync(this.logFile)) {
      fs.writeFileSync(this.logFile, "");
    }
  }

  public log(level: string, message: string) {
    const timestamp = new Date().toISOString();
    const logEntry = `${timestamp} [${level.toUpperCase()}]: ${message}\n`;
    fs.appendFileSync(this.logFile, logEntry);
  }

  public info(message: string) {
    this.log("info", message);
  }

  public warn(message: string) {
    this.log("warn", message);
  }

  public error(message: string) {
    this.log("error", message);
  }
}
