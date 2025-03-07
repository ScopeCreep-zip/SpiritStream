import fs from "fs";
import path from "path";
import { app } from "electron";

const LOG_LEVELS = ["error", "warn", "info", "debug"] as const;
type LogLevel = (typeof LOG_LEVELS)[number];

export class Logger {
    private static instance: Logger;
    private logDir: string;
    private defaultLogFile: string;
    private currentLogLevel: LogLevel;

    private constructor(userDataPath: string) {
        this.logDir = path.join(userDataPath, "logs");
        this.defaultLogFile = "app.log";
        this.currentLogLevel = "info";
        this.setupLogFile(this.defaultLogFile);
    }

    public static getInstance(): Logger {
        if (!Logger.instance) {
            const userDataPath = app.getPath("userData");
            Logger.instance = new Logger(userDataPath);
        }
        return Logger.instance;
    }

    private setupLogFile(logFile: string): void {
        const logFilePath = path.join(this.logDir, logFile);

        if (!fs.existsSync(this.logDir)) {
            fs.mkdirSync(this.logDir, { recursive: true });
            console.info(`Created logs directory: ${this.logDir}`);
        }

        if (!fs.existsSync(logFilePath)) {
            fs.writeFileSync(logFilePath, "");
            this.info(`Created log file: ${logFilePath}`);
        }
    }

    private getFormattedTimestamp(): string {
        const now = new Date();
        const offset = -now.getTimezoneOffset();
        const offsetHours = Math.floor(Math.abs(offset) / 60)
            .toString()
            .padStart(2, "0");
        const offsetMinutes = (Math.abs(offset) % 60)
            .toString()
            .padStart(2, "0");
        const offsetSign = offset >= 0 ? "+" : "-";

        return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}T` +
               `${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}:${String(now.getSeconds()).padStart(2, "0")}` +
               `.${String(now.getMilliseconds()).padStart(3, "0")}${offsetSign}${offsetHours}:${offsetMinutes}`;
    }

    private formatLogMessage(level: string, message: string): string {
      const timestamp = this.getFormattedTimestamp();
      return `[${timestamp}] [${level.toUpperCase()}] ${message}`;
    }

    private log(level: LogLevel, message: string, logFile: string = this.defaultLogFile): void {
        
        if (LOG_LEVELS.indexOf(level) > LOG_LEVELS.indexOf(this.currentLogLevel)) {
          return;
        }

        const logFilePath = path.join(this.logDir, logFile);
        const formattedMessage = `${message}\n`;

        try {
            fs.appendFileSync(logFilePath, formattedMessage, { encoding: "utf8" });
        } catch (err) {
            console.error(`[ERROR] Failed to write log to ${logFilePath}: ${err}`);
        }

        // Handle console output based on log level
        switch (level) {
            case "debug":
                console.debug(formattedMessage);
                break;
            case "info":
                console.info(formattedMessage);
                break;
            case "warn":
                console.warn(formattedMessage);
                break;
            case "error":
                console.error(formattedMessage);
                break;
        }
    }

    public debug(message: string, logFile?: string): void {
      const formattedMessage = this.formatLogMessage("debug", message);
      this.log("debug", formattedMessage, logFile);
    }

    public info(message: string, logFile?: string): void {
      const formattedMessage = this.formatLogMessage("info", message);
      this.log("info", formattedMessage, logFile);
    }

    public warn(message: string, logFile?: string): void {
      const formattedMessage = this.formatLogMessage("warn", message);
    }

    public error(message: string, logFile?: string): void {
      const formattedMessage = this.formatLogMessage("error", message);
      this.log("error", formattedMessage, logFile);
    }
}
