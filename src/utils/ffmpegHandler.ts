import path from "path";
import { app } from "electron";
import { spawn, ChildProcessWithoutNullStreams } from "child_process";
import { Logger } from "./logger";

export class FFmpegHandler {
    private static instance: FFmpegHandler;
    private ffmpegPath: string;
    private logger: Logger;

    private constructor() {
        this.ffmpegPath = path.join(
            app.getAppPath(),
            "resources",
            "ffmpeg",
            "bin",
            process.platform === "win32" ? "ffmpeg.exe" : "ffmpeg"
        );
        this.logger = Logger.getInstance();
    }

    public static getInstance(): FFmpegHandler {
        if (!FFmpegHandler.instance) {
            FFmpegHandler.instance = new FFmpegHandler();
        }
        return FFmpegHandler.instance;
    }

    public testFFmpeg(): void {
        const process: ChildProcessWithoutNullStreams = spawn(this.ffmpegPath, ["-version"]);

        process.stdout.on("data", (data) => {
            this.logger.info(`FFmpeg Output: ${data.toString().trim()}`, 'ffmpeg.log');
        });

        process.stderr.on("data", (data) => {
            this.logger.error(`FFmpeg Error: ${data.toString().trim()}`, 'ffmpeg.log');
        });

        process.on("close", (code) => {
            this.logger.info(`FFmpeg process exited with code ${code}`, 'ffmpeg.log');
        });
    }
}