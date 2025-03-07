import "./init";
import { app, BrowserWindow } from "electron";
import Fastify from "fastify";
import { Logger } from "../utils/logger";
import { FFmpegHandler } from "../utils/ffmpegHandler";
import { OutputGroup } from "../models/OutputGroup";
import { StreamTarget } from "../models/StreamTarget";
import path from "path"; 

const logger = Logger.getInstance();
const PORT = Number(process.env.FASTIFY_PORT) || 3000; // Allow configurable port

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

// Function to test FFmpeg streaming with dummy OutputGroups
function testFFmpegStreaming() {
    const ffmpegHandler = FFmpegHandler.getInstance();
    
    // Dummy input URL (could be a local file or network stream)
    const inputFileName = "test.mp4";
    const inputURL = app.isPackaged
      ? path.join(process.resourcesPath, inputFileName)  // Production (Electron packaged)
      : path.join(__dirname, "../..", inputFileName);       // Development (during `npm run dev`)
    logger.debug(`Using input file: ${inputURL}`);

  //   // Dummy output groups
  //   const outputGroups: OutputGroup[] = [
  //       new OutputGroup(
  //           "group1",
  //           "1080p60",
  //           "libx264",
  //           "1920x1080",
  //           "6000k",
  //           "60",
  //           "aac",
  //           "128k",
  //           true
  //       ),
  //       new OutputGroup(
  //           "group2",
  //           "720p30",
  //           "libx264",
  //           "1280x720",
  //           "3000k",
  //           "30",
  //           "aac",
  //           "128k",
  //           false
  //       ),
  //   ];
  
  // // Assign test StreamTargets with explicit RTMP ports
  // outputGroups[0].addStreamTarget(new StreamTarget("yt", "rtmp://localhost/test", "KEY1", 1936)); // YouTube on 1936
  // outputGroups[0].addStreamTarget(new StreamTarget("fb", "rtmp://localhost/test", "KEY2", 1937)); // Facebook on 1937

  // outputGroups[1].addStreamTarget(new StreamTarget("twitch", "rtmp://localhost/test", "KEY3", 1938)); // Twitch on 1938
  // outputGroups[1].addStreamTarget(new StreamTarget("kick", "rtmp://localhost/test", "KEY4", 1939)); // Kick on 1939

  // Dummy output group 
  const outputGroups: OutputGroup[] = [
    new OutputGroup(
        "group1",
        "1080p30",
        "h264_qsv",  // Using libx264 for H.264 encoding
        "1920x1080",  // 1080p resolution
        "3000k",  // Lower bitrate for less resource usage
        "30",  // 30fps for lower resource consumption
        "aac",  // Audio codec AAC
        "128k",  // Audio bitrate
        false  // Disable PTS flag for simplicity
    )
  ];

  // Assign test StreamTarget with explicit RTMP port
  outputGroups[0].addStreamTarget(new StreamTarget("yt", "rtmp://localhost/test", "KEY1", 1936));  // YouTube on 1936


  logger.info("Starting FFmpeg test with dummy OutputGroups...");
  ffmpegHandler.startFFmpeg(inputURL, outputGroups);
}

// Start Fastify when Electron is ready
app.whenReady().then(async () => {
  try {
    logger.debug("Testing FFmpeg...");
    const ffmpegHandler = FFmpegHandler.getInstance();
    
    try {
      ffmpegHandler.testFFmpeg();
      logger.debug("FFmpeg test successful.");
    } catch (err) {
      logger.error(`FFmpeg test failed: ${err}`);
    }

    logger.debug("Attempting to start Fastify...");
    await fastify.listen({ port: PORT });
    logger.debug(`Fastify running on http://localhost:${PORT}`);

    ffmpegHandler.getAvailableAudioEncoders()
      .then(encoders => {
        logger.debug("Available Audio Encoders:");
        encoders.forEach(encoder => logger.debug(encoder));
      })
      .catch(error => {
        logger.error(`Error detecting available audio encoders: ${error}`);
      });

    ffmpegHandler.getAvailableVideoEncoders()
      .then(encoders => {
        logger.debug("Available Video Encoders:");
        encoders.forEach(encoder => logger.debug(encoder));
      })
      .catch(error => {
        logger.error(`Error detecting available video encoders: ${error}`);
      }); 

    // Start FFmpeg streaming test after everything else is initialized
    testFFmpegStreaming();

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
