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
        this.currentLogLevel = "debug";
        this.setupLogFile(this.defaultLogFile);
    }

    public static async getInstance(): Promise<Logger> {
        if (!Logger.instance) {
            await app.whenReady();
            const userDataPath = app.getPath("userData");
            Logger.instance = new Logger(userDataPath);
        }
        return Logger.instance;
    }

    private setupLogFile(logFile: string): void {
        const logFilePath = path.join(this.logDir, logFile);

        if (!fs.existsSync(this.logDir)) {
            fs.mkdirSync(this.logDir, { recursive: true });
            this.internalLog("info", `Created logs directory: ${this.logDir}`);
        }

        if (!fs.existsSync(logFilePath)) {
            fs.writeFileSync(logFilePath, "");
            this.internalLog("info", `Created log file: ${logFilePath}`);
        }
    }

    private internalLog(level: LogLevel, message: string): void {
        console[level](`[${level.toUpperCase()}]`, message);
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

    private log(level: LogLevel, message: string, logFile: string = this.defaultLogFile): void {
        if (LOG_LEVELS.indexOf(level) > LOG_LEVELS.indexOf(this.currentLogLevel)) {
            return;
        }

        const logFilePath = path.join(this.logDir, logFile);
        const timestamp = this.getFormattedTimestamp(); // Use the new private method
        const formattedMessage = `[${timestamp}] [${level.toUpperCase()}] ${message}\r\n`;

        try {
            fs.appendFileSync(logFilePath, formattedMessage, { encoding: "utf8" });
        } catch (err) {
            this.internalLog("error", `Failed to write log to ${logFilePath}: ${err}`);
        }
    }

    public debug(message: string, logFile?: string): void {
        this.log("debug", message, logFile);
    }

    public info(message: string, logFile?: string): void {
        this.log("info", message, logFile);
    }

    public warn(message: string, logFile?: string): void {
        this.log("warn", message, logFile);
    }

    public error(message: string, logFile?: string): void {
        this.log("error", message, logFile);
    }

    public setLogLevel(level: LogLevel): void {
        if (LOG_LEVELS.includes(level)) {
            this.currentLogLevel = level;
        } else {
            this.internalLog("warn", `Invalid log level: ${level}, defaulting to 'warn'`);
            this.currentLogLevel = "warn";
        }
    }
}

// Keep the export name `logger` but make it a Promise
export const logger = Logger.getInstance();
