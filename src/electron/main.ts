import "./init";
import { app, BrowserWindow } from "electron";
import { FastifyServer } from "../server/FastifyServer";  
import { Logger } from "../utils/logger";

const logger = Logger.getInstance();
const server = new FastifyServer();  

// Ensure main window is recreated if all windows are closed (macOS behavior)
app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) {
        new BrowserWindow({
            width: 800,
            height: 600,
            webPreferences: {
                nodeIntegration: true,
            },
        }).loadURL("about:blank");
    }
});

// Start Fastify when Electron is ready
app.whenReady().then(() => {
    try {
        server.start();  
    } catch (err) {
        logger.error(`Fastify failed to start: ${err}`);
        process.exit(1);
    }

    const mainWindow = new BrowserWindow({
        width: 800,
        height: 600,
        webPreferences: {
            nodeIntegration: true, // May change if using a preload script later
        },
    });

    mainWindow.loadURL("about:blank"); // Temporary, will change later
});
