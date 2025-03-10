const { app, BrowserWindow } = require("electron");
const path = require("path");

let mainWindow;

app.whenReady().then(() => {
    mainWindow = new BrowserWindow({
        width: 1200,  
        height: 800,  
        fullscreen: false,  // Do NOT use exclusive fullscreen
        frame: true,        // Keep the title bar (default behavior)
        maximizable: true,  // Allows maximizing
        resizable: true,    // Allows resizing
        webPreferences: {
            nodeIntegration: true,
        },
    });

    mainWindow.maximize(); // Start the window in maximized mode
    mainWindow.loadFile(path.join(__dirname, "../frontend/index/html/index.html"));
});
