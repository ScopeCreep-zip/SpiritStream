import { ipcMain } from "electron";
import { ProfileManager } from "../utils/profileManager";
import { FFmpegHandler } from "../utils/ffmpegHandler";
import { reconstructOutputGroups, reconstructTheme } from "../utils/dtoUtils";
import { Profile } from "../models/Profile";
import { ProfileDTO, OutputGroupDTO } from "../shared/interfaces";

// Instantiate backends
const profileManager = ProfileManager.getInstance();
const ffmpegHandler = FFmpegHandler.getInstance();

export function registerIPCHandlers() {
  // Profile Manager
  ipcMain.handle("profile:getAllProfileNames", () => {
    return profileManager.getAllProfileNames();
  });

  ipcMain.handle("profile:load", (_event, name: string, password?: string) => {
    const profile = profileManager.loadProfile(name, password);
    return profile ? profile.toDTO() : null;
  });

  ipcMain.handle("profile:save", (_event, dto: ProfileDTO, password?: string) => {
    const profile = new Profile(
      dto.id,
      dto.name,
      dto.incomingURL,
      dto.outputGroups.some(g => g.generatePTS),
      reconstructTheme(dto.theme)
    );
    reconstructOutputGroups(dto.outputGroups).forEach(group => profile.addOutputGroup(group));
    profileManager.saveProfile(profile, password);
  });

  ipcMain.handle("profile:delete", (_event, name: string) => {
    profileManager.deleteProfile(name);
  });

  ipcMain.handle("profile:getLastUsed", () => {
    return profileManager.getLastUsedProfile();
  });

  ipcMain.handle("profile:saveLastUsed", (_event, name: string) => {
    profileManager.saveLastUsedProfile(name);
  });

  // FFmpeg Handler
  ipcMain.handle("ffmpeg:test", () => ffmpegHandler.testFFmpeg());

  ipcMain.handle("ffmpeg:start", (_event, inputURL: string, groups: OutputGroupDTO[], testMode?: boolean) => {
    const reconstructed = reconstructOutputGroups(groups);
    ffmpegHandler.startFFmpeg(inputURL, reconstructed, testMode);
  });

  ipcMain.handle("ffmpeg:stop", (_event, groupId: string) => ffmpegHandler.stopFFmpeg(groupId));
  ipcMain.handle("ffmpeg:stopAll", () => ffmpegHandler.stopAllFFmpeg());

  ipcMain.handle("ffmpeg:getAudioEncoders", () => ffmpegHandler.getAvailableAudioEncoders());
  ipcMain.handle("ffmpeg:getVideoEncoders", () => ffmpegHandler.getAvailableVideoEncoders());
}
