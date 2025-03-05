import "./init";
import { app, BrowserWindow, ipcMain } from "electron";
import path from "path";
import { Logger } from "../utils/logger";
import { ProfileManager } from "../utils/profileManager";

const mainDir = path.resolve(__dirname, "../..");
const logger = Logger.getInstance();
const profileManager = ProfileManager.getInstance();

let mainWindow: BrowserWindow | null = null;

app.whenReady().then(() => {
    logger.info("Initializing Profile Manager...");
    console.log(profileManager.getAllProfileNames()); 
    profileManager.getAllProfileNames();


    logger.info("Creating main application window...");
    createWindow();
});

const createWindow = () => {
    logger.info("Creating main application window...");

    mainWindow = new BrowserWindow({
        width: 1200,
        height: 800,
        webPreferences: {
            nodeIntegration: true,
            contextIsolation: false,
        },
    });

    if (process.env.NODE_ENV === "development") {
        const loadURL = () => {
            try {
                logger.info("Loading Vite development server...");
                mainWindow?.loadURL("http://localhost:5173");
                mainWindow?.webContents.openDevTools();
            } catch (e) {
                logger.warn("Vite dev server not ready, retrying in 1s...");
                setTimeout(loadURL, 1000);
            }
        };
        loadURL();
    } else {
        logger.info("Loading production build...");
        mainWindow.loadURL(`file://${path.join(mainDir, "dist/index.html")}`);
    }
};

app.on("window-all-closed", () => {
    logger.warn("All windows closed.");
    if (process.platform !== "darwin") {
        logger.info("Quitting app...");
        app.quit();
    }
});

app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) {
        logger.info("Recreating window on macOS activate.");
        createWindow();
    }
});
