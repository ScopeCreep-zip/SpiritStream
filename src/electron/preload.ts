import { contextBridge, ipcRenderer } from "electron";

contextBridge.exposeInMainWorld("electronAPI", {
  getUserDataPath: () => ipcRenderer.sendSync("get-user-data-path"),

  profileManager: {
    getAllProfileNames: () => ipcRenderer.invoke("profile:getAllProfileNames"),
    loadProfile: (name: string, password?: string) => ipcRenderer.invoke("profile:load", name, password),
    saveProfile: (profile: any, password?: string) => ipcRenderer.invoke("profile:save", profile, password),
    deleteProfile: (name: string) => ipcRenderer.invoke("profile:delete", name),
    getLastUsedProfile: () => ipcRenderer.invoke("profile:getLastUsed"),
    saveLastUsedProfile: (name: string) => ipcRenderer.invoke("profile:setLastUsed", name)
  },

  ffmpegHandler: {
    testFFmpeg: () => ipcRenderer.invoke("ffmpeg:test"),
    startFFmpeg: (inputURL: string, outputGroups: any[], testMode?: boolean) =>
      ipcRenderer.invoke("ffmpeg:start", inputURL, outputGroups, testMode),
    stopFFmpeg: (groupId: string) => ipcRenderer.invoke("ffmpeg:stop", groupId),
    stopAllFFmpeg: () => ipcRenderer.invoke("ffmpeg:stopAll"),
    getAvailableAudioEncoders: () => ipcRenderer.invoke("ffmpeg:getAudioEncoders"),
    getAvailableVideoEncoders: () => ipcRenderer.invoke("ffmpeg:getVideoEncoders")
  },

  logger: {
    info: (msg: string) => ipcRenderer.send("renderer-log", { level: "info", message: msg, logFile: "frontend.log" }),
    error: (msg: string) => ipcRenderer.send("renderer-log", { level: "error", message: msg, logFile: "frontend.log" }),
  }
});
