import { app, BrowserWindow, ipcMain } from "electron";
import path from "path";
import { registerIPCHandlers } from "./ipcHandlers";

let mainWindow: BrowserWindow;

app.whenReady().then(() => {
  // Configure application name and userData path
  app.setName("MagillaStream");
  app.setPath("userData", path.join(app.getPath("appData"), "MagillaStream"));

  // Register IPC handlers
  registerIPCHandlers();

  // Create the main application window
  mainWindow = new BrowserWindow({
    width: 1200,
    height: 800,
    fullscreen: false,
    frame: true,
    maximizable: true,
    resizable: true,
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
      sandbox: true,
      preload: path.join(__dirname, "preload.js"),
    },
  });

  mainWindow.maximize();
  mainWindow.loadFile(path.join(__dirname, "../frontend/index/index.html"));
});

// Synchronous IPC response for user data path
ipcMain.on("get-user-data-path", (event) => {
  event.returnValue = app.getPath("userData");
});
