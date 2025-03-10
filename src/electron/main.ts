import "./init";
import { app, BrowserWindow } from "electron";  
import { Logger } from "../utils/logger";
import * as path from "path";

const logger = Logger.getInstance();

// Ensure main window is recreated if all windows are closed (macOS behavior)
app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) {
        createMainWindow();
    }
});

// Function to create the main Electron window
function createMainWindow() {
    const mainWindow = new BrowserWindow({
        width: 1280,
        height: 800,
        webPreferences: {
            nodeIntegration: true, // May change if using a preload script later
        },
        title: "MagillaStream"
    });

    // Load local index.html in both development and production
    mainWindow.loadFile(path.join(__dirname, "../frontend/index/html/index.html"));

    return mainWindow;
}

// Start Fastify when Electron is ready
app.whenReady().then(() => {

    createMainWindow();
});
