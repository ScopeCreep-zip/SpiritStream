import "./init";
import { app, BrowserWindow } from "electron";
import Fastify from "fastify";
import { Logger } from "../utils/logger"; 

const logger = Logger.getInstance(); 

// Create Fastify server
const fastify = Fastify({
  logger: {
    level: "info",
    stream: {
      write: (msg) => logger.info(msg.trim()), // Pipe Fastify logs to logger
    },
  },
});

// Test route
fastify.get("/api/test", async (request, reply) => {
  return { message: "Fastify is running inside Electron!" };
});

// Start Fastify when Electron is ready
app.whenReady().then(async () => {
  try {
    logger.info("Attempting to start Fastify..."); 
    await fastify.listen({ port: 3000 });
    logger.info("Fastify running on http://localhost:3000");
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
