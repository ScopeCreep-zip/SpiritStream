import { spawn, ChildProcess } from "child_process";
import { Logger } from "./logger";
import * as fs from 'fs';
import * as path from 'path';
import { app } from 'electron';

export class EncoderDetection {
    private logger: Logger;
    private ffmpegPath: string;
    private videoEncodersWhitelist: string[] = [];
    private audioEncodersWhitelist: string[] = [];

    constructor(ffmpegPath: string) {
        this.logger = Logger.getInstance();
        this.ffmpegPath = ffmpegPath;

        // Load the encoder configuration file from the 'config' directory
        this.loadConfig();
    }

    // Load the config file from 'config/encoders.conf'
    private loadConfig(): void {
        const isDev = process.env.NODE_ENV === 'development';  
        let configDir;
        
        if (isDev) {
            // In development, ensure we are looking in the root 'dist' directory
            configDir = path.join(process.cwd(), 'dist', 'config');  // Use process.cwd() to get the root path
            this.logger.info(`Development mode: Looking for config in ${configDir}`, 'ffmpeg.log');
        } else {
            // In production, use the userData directory for config
            configDir = path.join(app.getPath('userData'), 'config');
            this.logger.info(`Production mode: Looking for config in ${configDir}`, 'ffmpeg.log');
        }
        
        const configFilePath = path.join(configDir, 'encoders.conf');  // Path to the config file
    
        this.logger.info(`Final config file path: ${configFilePath}`, 'ffmpeg.log');
    
        // Check if the config directory exists
        if (fs.existsSync(configDir)) {
            // Check if the config file exists
            if (fs.existsSync(configFilePath)) {
                try {
                    // Read the config file
                    const config = JSON.parse(fs.readFileSync(configFilePath, 'utf-8'));
                    this.videoEncodersWhitelist = config.videoEncoders || [];
                    this.audioEncodersWhitelist = config.audioEncoders || [];
                    this.logger.info("Loaded config from encoders.conf.", 'ffmpeg.log');
                } catch (error) {
                    this.logger.error("Error loading encoders.conf", 'ffmpeg.log');
                }
            } else {
                this.logger.debug(`Config file 'encoders.conf' not found at ${configFilePath}, using default encoders.`, 'ffmpeg.log');
                this.setDefaultWhitelists();
            }
        } else {
            this.logger.warn(`Config directory not found at ${configDir}, using default encoders.`, 'ffmpeg.log');
            this.setDefaultWhitelists();
        }
    }    

    // Fallback to default whitelist values if the config file doesn't exist or is malformed
    private setDefaultWhitelists(): void {
        this.videoEncodersWhitelist = [
            "libx264", "libx265", "nvenc", "qsv", "av1", "libvpx", "vulkan"
        ];
        this.audioEncodersWhitelist = [
            "aac", "opus", "libmp3lame"
        ];
    }

    // Public method for getting video encoders
    public getAvailableVideoEncoders(showAllEncoders = false): Promise<string[]> {
        return new Promise((resolve, reject) => {
            const videoEncoders: string[] = [];
            const encoderProcess: ChildProcess = spawn(this.ffmpegPath, ["-encoders"]);

            encoderProcess.stdout?.on("data", (data) => {
                const output = data.toString();
                this.logger.info(`FFmpeg Encoder Output: ${output.trim()}`, 'ffmpeg.log');
                
                // Extract video encoders (lines starting with ' V')
                const videoCodecs = output
                    .split("\n")
                    .filter((line: string) => line.startsWith(" V")) // Keep only video encoders
                    .map((line: string) => {
                        const parts = line.trim().split(/\s+/); // Use regex to handle multiple spaces
                        return parts.length > 1 ? parts[1] : ""; // Return empty string instead of null
                    })
                    .filter((encoder: string) => encoder.length > 0); // Remove empty entries

                
                // If showAllEncoders is false, filter based on the whitelist
                const filteredVideoCodecs = showAllEncoders
                    ? videoCodecs
                    : videoCodecs.filter((encoder: string) => 
                        encoder && this.videoEncodersWhitelist.some(allowed => encoder.includes(allowed))
                    );

                videoEncoders.push(...filteredVideoCodecs);
            });

            encoderProcess.stderr?.on("data", (data) => {
                const errorOutput = data.toString().trim();
                if (errorOutput.includes("Error")) {
                    this.logger.error(`FFmpeg Error: ${errorOutput}`, 'ffmpeg.log');
                }
            });

            encoderProcess.on("close", (code) => {
                if (code !== 0) {
                    reject(`FFmpeg encoder process failed with code ${code}`);
                } else {
                    resolve(videoEncoders);
                }
            });
        });
    }

    // Public method for getting audio encoders
    public getAvailableAudioEncoders(showAllEncoders = false): Promise<string[]> {
        return new Promise((resolve, reject) => {
            const audioEncoders: string[] = [];
            const encoderProcess: ChildProcess = spawn(this.ffmpegPath, ["-encoders"]);

            encoderProcess.stdout?.on("data", (data) => {
                const output = data.toString();
                this.logger.info(`FFmpeg Encoder Output: ${output.trim()}`, 'ffmpeg.log');
                
                // Extract audio encoders (lines starting with ' A')
                const audioCodecs = output
                    .split("\n")
                    .filter((line: string) => line.startsWith(" A")) // Keep only audio encoders
                    .map((line: string) => {
                        const parts = line.trim().split(/\s+/); // Use regex to handle multiple spaces
                        return parts.length > 1 ? parts[1] : ""; // Return empty string instead of null
                    })
                    .filter((encoder: string) => encoder.length > 0); // Remove empty entries

                
                // If showAllEncoders is false, filter based on the whitelist
                const filteredAudioCodecs = showAllEncoders 
                    ? audioCodecs 
                    : audioCodecs.filter((encoder: string) => 
                        encoder && this.audioEncodersWhitelist.includes(encoder)
                    );

                audioEncoders.push(...filteredAudioCodecs);
            });

            encoderProcess.stderr?.on("data", (data) => {
                const errorOutput = data.toString().trim();
                if (errorOutput.includes("Error")) {
                    this.logger.error(`FFmpeg Error: ${errorOutput}`, 'ffmpeg.log');
                }
            });

            encoderProcess.on("close", (code) => {
                if (code !== 0) {
                    reject(`FFmpeg encoder process failed with code ${code}`);
                } else {
                    resolve(audioEncoders);
                }
            });
        });
    }
}
