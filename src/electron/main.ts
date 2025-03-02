import "./init";
import { app, BrowserWindow, ipcMain } from "electron";
import path from "path";
import { logger } from "../utils/logger";

const mainDir = path.resolve(__dirname, "../..");

let mainWindow: BrowserWindow | null = null;

app.whenReady().then(async () => {
    const log = await logger;
    log.info("Creating main application window...");

    createWindow();
});

const createWindow = async () => {
    const log = await logger;
    log.info("Creating main application window...");

    mainWindow = new BrowserWindow({
        width: 1200,
        height: 800,
        webPreferences: {
            nodeIntegration: true,
            contextIsolation: false,
        },
    });

    if (process.env.NODE_ENV === "development") {
        const loadURL = async () => {
            try {
                log.info("Loading Vite development server...");
                await mainWindow?.loadURL("http://localhost:5173");
                mainWindow?.webContents.openDevTools();
            } catch (e) {
                log.warn("Vite dev server not ready, retrying in 1s...");
                setTimeout(loadURL, 1000);
            }
        };
        loadURL();
    } else {
        log.info("Loading production build...");
        mainWindow.loadURL(`file://${path.join(mainDir, "dist/index.html")}`);
    }
};

app.on("window-all-closed", async () => {
    const log = await logger;
    log.warn("All windows closed.");
    if (process.platform !== "darwin") {
        log.info("Quitting app...");
        app.quit();
    }
});

app.on("activate", async () => {
    const log = await logger;
    if (BrowserWindow.getAllWindows().length === 0) {
        log.info("Recreating window on macOS activate.");
        createWindow();
    }
});
