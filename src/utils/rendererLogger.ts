import { ipcRenderer } from "electron";

type LogLevel = "error" | "warn" | "info" | "debug";

/**
 * Logger for Electron renderer process (frontend)
 */
export const RendererLogger = {
  error: (message: string) => ipcRenderer.send("renderer-log", { level: "error", message, logFile: "frontend.log" }),
  warn: (message: string) => ipcRenderer.send("renderer-log", { level: "warn", message, logFile: "frontend.log" }),
  info: (message: string) => ipcRenderer.send("renderer-log", { level: "info", message, logFile: "frontend.log" }),
  debug: (message: string) => ipcRenderer.send("renderer-log", { level: "debug", message, logFile: "frontend.log" }),
};
