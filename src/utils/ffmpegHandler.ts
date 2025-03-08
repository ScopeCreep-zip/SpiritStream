import path from "path";
import { app } from "electron";
import { spawn, ChildProcess, ChildProcessWithoutNullStreams } from "child_process";
import { Logger } from "./logger";
import { EncoderDetection } from "./encoderDetection";
import { OutputGroup } from "../models/OutputGroup";
import { StreamTarget } from "../models/StreamTarget";

export class FFmpegHandler {
    private static instance: FFmpegHandler;
    private ffmpegPath: string;
    private logger: Logger;
    private encoderDetection: EncoderDetection;
    private runningProcesses: Map<string, ChildProcess>; // Track running FFmpeg processes

    private constructor() {
        // Determine the FFmpeg binary path based on environment
        if (app.isPackaged) {
            this.ffmpegPath = path.join(
                process.resourcesPath,
                "ffmpeg",
                "bin",
                process.platform === "win32" ? "ffmpeg.exe" : "ffmpeg"
            );
        } else {
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
        this.runningProcesses = new Map();
    }

    public static getInstance(): FFmpegHandler {
        if (!FFmpegHandler.instance) {
            FFmpegHandler.instance = new FFmpegHandler();
        }
        return FFmpegHandler.instance;
    }

    // Runs a basic FFmpeg version test to verify functionality  
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

    // Generates an FFmpeg command string for a given OutputGroup
    public createFFmpegCommand(inputURL: string, outputGroup: OutputGroup, testMode = false): string {
        let command = `${this.ffmpegPath} -loglevel warning`;
    
        // If in test mode, add real-time processing flag *before* input
        if (testMode) {
            command += " -re";
        }
    
        command += ` -i "${inputURL}"`;
    
        // Set video and audio encoding parameters
        command += ` -c:v ${outputGroup.getVideoEncoder()}`;
        command += ` -b:v ${outputGroup.getBitrate()}`;
        command += ` -r ${outputGroup.getFps()}`;
    
        // If in test mode, add GOP size after video codec (correct placement)
        if (testMode) {
            command += " -g 50";
        }
    
        command += ` -c:a ${outputGroup.getAudioCodec()}`;
        command += ` -b:a ${outputGroup.getAudioBitrate()}`;
    
        // Use -map to send the same encoded stream to multiple StreamTargets
        command += ` -map 0:v -map 0:a`;
    
        // Add each StreamTarget as an output
        outputGroup.getStreamTargets().forEach((target: StreamTarget) => {
            const targetUrl = target.normalizedPath;
            command += ` -f flv "${targetUrl}"`;
        });
    
        this.logger.info(`Generated FFmpeg Command: ${command}`, 'ffmpeg.log');
        return command;
    }
    
    // Starts FFmpeg processes for all OutputGroups
    public startFFmpeg(inputURL: string, outputGroups: OutputGroup[], testMode: boolean = false): void {
        outputGroups.forEach(group => {
            const command = this.createFFmpegCommand(inputURL, group, testMode);
            const process = this.runFFmpegCommand(group.getId(), command);
            this.runningProcesses.set(group.getId(), process);
        });
    }
    

    // Runs a single FFmpeg process for an OutputGroup
    private runFFmpegCommand(groupId: string, command: string): ChildProcess {
        this.logger.info(`Starting FFmpeg for OutputGroup ${groupId}: ${command}`, 'ffmpeg.log');

        const process = spawn(command, { shell: true });

        process.stdout.on("data", (data) => {
            this.logger.info(`[${groupId}] FFmpeg Output: ${data.toString().trim()}`, 'ffmpeg.log');
        });

        process.stderr.on("data", (data) => {
            this.logger.error(`[${groupId}] FFmpeg Error: ${data.toString().trim()}`, 'ffmpeg.log');
        });

        process.on("close", (code) => {
            this.logger.info(`[${groupId}] FFmpeg exited with code ${code}`, 'ffmpeg.log');
            this.runningProcesses.delete(groupId); // Remove from tracking when it exits
        });

        return process;
    }

    // Stops an individual FFmpeg process for a specific OutputGroup
    public stopFFmpeg(groupId: string): void {
        if (this.runningProcesses.has(groupId)) {
            this.runningProcesses.get(groupId)?.kill();
            this.runningProcesses.delete(groupId);
            this.logger.info(`Stopped FFmpeg process for OutputGroup ${groupId}`, 'ffmpeg.log');
        } else {
            this.logger.warn(`No FFmpeg process found for OutputGroup ${groupId}`, 'ffmpeg.log');
        }
    }

    // Stops all running FFmpeg processes
    public stopAllFFmpeg(): void {
        this.runningProcesses.forEach((process, groupId) => {
            process.kill();
            this.logger.info(`Stopped FFmpeg process for OutputGroup ${groupId}`, 'ffmpeg.log');
        });

        this.runningProcesses.clear();
    }

    // Retrieves available audio encoders
    public async getAvailableAudioEncoders(): Promise<string[]> {
        return this.encoderDetection.getAvailableAudioEncoders();
    }

    // Retrieves available video encoders
    public async getAvailableVideoEncoders(): Promise<string[]> {
        return this.encoderDetection.getAvailableVideoEncoders();
    }
}
