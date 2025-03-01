import { app, BrowserWindow } from "electron";
import { dirname } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));

let mainWindow;

app.whenReady().then(() => {
  mainWindow = new BrowserWindow({
    width: 1000,
    height: 700,
    webPreferences: {
      nodeIntegration: true,
    },
  });

  // Wait 2 seconds for Vite to be ready before loading
  setTimeout(() => {
    mainWindow.loadURL("http://localhost:5173");
  }, 2000);

  mainWindow.webContents.openDevTools(); // Open DevTools to debug

  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
});

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") {
    app.quit();
  }
});
