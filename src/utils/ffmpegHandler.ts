import path from "path";
import { app } from "electron";
import { spawn, ChildProcessWithoutNullStreams } from "child_process";
import { Logger } from "./logger";
import { EncoderDetection } from "./encoderDetection";

export class FFmpegHandler {
    private static instance: FFmpegHandler;
    private ffmpegPath: string;
    private logger: Logger;
    private encoderDetection: EncoderDetection;

    private constructor() {
        // Get the correct path to the FFmpeg binary based on env
        if (app.isPackaged) {
            // production path
            this.ffmpegPath = path.join(
              process.resourcesPath,
              "ffmpeg",
              "bin",
              process.platform === "win32" ? "ffmpeg.exe" : "ffmpeg"
            );
          } else {
            // development path
            this.ffmpegPath = path.join(
              path.dirname(app.getAppPath()), 
              "resources",
              "ffmpeg",
              "bin",
              process.platform === "win32" ? "ffmpeg.exe" : "ffmpeg"
            );
          }

        this.logger = Logger.getInstance();
        this.encoderDetection = new EncoderDetection(this.ffmpegPath);
    }

    public static getInstance(): FFmpegHandler {
        if (!FFmpegHandler.instance) {
            FFmpegHandler.instance = new FFmpegHandler();
        }
        return FFmpegHandler.instance;
    }

    public testFFmpeg(): void {
        this.logger.info(`Testing FFmpeg path at: ${this.ffmpegPath}`, 'ffmpeg.log');
        const process: ChildProcessWithoutNullStreams = spawn(this.ffmpegPath, ["-version"]);

        process.stdout.on("data", (data) => {
            this.logger.info(`Using FFmpeg at: ${this.ffmpegPath}`, 'ffmpeg.log');
        this.logger.info(`FFmpeg Output: ${data.toString().trim()}`, 'ffmpeg.log');
        });

        process.stderr.on("data", (data) => {
            this.logger.error(`FFmpeg Error: ${data.toString().trim()}`, 'ffmpeg.log');
        });

        process.on("close", (code) => {
            this.logger.info(`FFmpeg process exited with code ${code}`, 'ffmpeg.log');
        });
    }

    public async getAvailableAudioEncoders(): Promise<string[]> {
        return this.encoderDetection.getAvailableAudioEncoders();
    }

    public async getAvailableVideoEncoders(): Promise<string[]> {
        return this.encoderDetection.getAvailableVideoEncoders();
    }
}
