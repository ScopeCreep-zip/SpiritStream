import Fastify, { FastifyInstance } from "fastify";
import { ServerBase } from "./ServerBase";
import { Profile } from "../models/Profile";
import { OutputGroup } from "../models/OutputGroup";
import { StreamTarget } from "../models/StreamTarget";
import { FFmpegHandler } from "../utils/ffmpegHandler";
import { Logger } from "../utils/logger";

export class FastifyServer extends ServerBase {
    private fastify: FastifyInstance;
    private logger: Logger;
    private ffmpegHandler: FFmpegHandler;

    constructor() {
        super();
        this.fastify = Fastify({ logger: false });
        this.logger = Logger.getInstance();
        this.ffmpegHandler = FFmpegHandler.getInstance();

        this.setupRoutes();
    }

    public start(): void {
        const port = Number(process.env.FASTIFY_PORT) || 3000;
        this.fastify.listen({ port }, (err, address) => {
            if (err) {
                this.logger.error(`Fastify failed to start: ${err.message}`);
                process.exit(1);
            }
            this.logger.info(`Fastify server running at ${address}`);
        });
    }

    public stop(): void {
        this.fastify.close().then(() => {
            this.logger.info("Fastify server stopped.");
        }).catch(err => {
            this.logger.error(`Error stopping Fastify: ${err.message}`);
        });
    }

    public test(): string {
        return "Server test executed successfully.";
    }

    private setupRoutes(): void {
        // Profiles
        this.fastify.get("/api/profiles", async (_, reply) => reply.send(this.getProfiles()));
        this.fastify.get<{ Params: { id: string } }>("/api/profiles/:id", async (req, reply) => 
            reply.send(this.getProfile(req.params.id))
        );
        this.fastify.post<{ Body: Profile }>("/api/profiles", async (req, reply) => {
            this.createProfile(req.body);
            reply.send({ message: "Profile created." });
        });
        this.fastify.put<{ Body: Profile }>("/api/profiles/:id", async (req, reply) => {
            this.updateProfile(req.body);
            reply.send({ message: "Profile updated." });
        });
        this.fastify.delete<{ Params: { id: string } }>("/api/profiles/:id", async (req, reply) => {
            this.deleteProfile(req.params.id);
            reply.send({ message: "Profile deleted." });
        });
    
        // Output Groups
        this.fastify.get("/api/output-groups", async (_, reply) => reply.send(this.getOutputGroups()));
        this.fastify.get<{ Params: { id: string } }>("/api/output-groups/:id", async (req, reply) => 
            reply.send(this.getOutputGroup(req.params.id))
        );
        this.fastify.post<{ Body: OutputGroup }>("/api/output-groups", async (req, reply) => {
            this.createOutputGroup(req.body);
            reply.send({ message: "Output group created." });
        });
        this.fastify.put<{ Body: OutputGroup }>("/api/output-groups/:id", async (req, reply) => {
            this.updateOutputGroup(req.body);
            reply.send({ message: "Output group updated." });
        });
        this.fastify.delete<{ Params: { id: string } }>("/api/output-groups/:id", async (req, reply) => {
            this.deleteOutputGroup(req.params.id);
            reply.send({ message: "Output group deleted." });
        });
    
        // Stream Targets
        this.fastify.get("/api/stream-targets", async (_, reply) => reply.send(this.getStreamTargets()));
        this.fastify.get<{ Params: { id: string } }>("/api/stream-targets/:id", async (req, reply) => 
            reply.send(this.getStreamTarget(req.params.id))
        );
        this.fastify.post<{ Body: StreamTarget }>("/api/stream-targets", async (req, reply) => {
            this.createStreamTarget(req.body);
            reply.send({ message: "Stream target created." });
        });
        this.fastify.put<{ Body: StreamTarget }>("/api/stream-targets/:id", async (req, reply) => {
            this.updateStreamTarget(req.body);
            reply.send({ message: "Stream target updated." });
        });
    
        // FFmpeg Control
        this.fastify.get("/api/ffmpeg/test", async (_, reply) => reply.send(this.testFFmpeg()));
        this.fastify.get("/api/ffmpeg/video-encoders", async (_, reply) => reply.send(this.getVideoEncoders()));
        this.fastify.get("/api/ffmpeg/audio-encoders", async (_, reply) => reply.send(this.getAudioEncoders()));
    
        this.fastify.post<{ Body: OutputGroup[] }>("/api/ffmpeg/start", async (req, reply) => {
            this.startFFmpeg(req.body);
            reply.send({ message: "FFmpeg started." });
        });
    
        this.fastify.post("/api/ffmpeg/stop", async (_, reply) => {
            this.stopFFmpeg();
            reply.send({ message: "FFmpeg stopped." });
        });
    }
    

    // Implement abstract methods
    public getProfiles(): Profile[] {
        return []; // Stub implementation
    }

    public getProfile(id: string): Profile {
        return new Profile(id, "Test Profile", "rtmp://localhost", false);
    }

    public createProfile(profile: Profile): void {
        this.logger.info("Creating profile:", profile.getName());
    }

    public updateProfile(profile: Profile): void {
        this.logger.info("Updating profile:", profile.getName());
    }

    public deleteProfile(id: string): void {
        this.logger.info("Deleting profile:", id);
    }

    public getOutputGroups(): OutputGroup[] {
        return [];
    }

    public getOutputGroup(id: string): OutputGroup {
        return new OutputGroup(id, "Example", "libx264", "1920x1080", "5000k", "30", "aac", "128k", false);
    }

    public createOutputGroup(outputGroup: OutputGroup): void {
        this.logger.info("Creating output group:", outputGroup.getName());
    }

    public updateOutputGroup(outputGroup: OutputGroup): void {
        this.logger.info("Updating output group:", outputGroup.getName());
    }

    public deleteOutputGroup(id: string): void {
        this.logger.info("Deleting output group:", id);
    }

    public getStreamTargets(): StreamTarget[] {
        return [];
    }

    public getStreamTarget(id: string): StreamTarget {
        return new StreamTarget(id, "rtmp://localhost/test", "KEY1", 1936);
    }

    public createStreamTarget(streamTarget: StreamTarget): void {
        this.logger.info("Creating stream target:", streamTarget.normalizedPath);
    }

    public updateStreamTarget(streamTarget: StreamTarget): void {
        this.logger.info("Updating stream target:", streamTarget.normalizedPath);
    }

    public testFFmpeg(): string {
        return "FFmpeg test executed.";
    }

    public getVideoEncoders(): string[] {
        return ["libx264", "h264_qsv", "nvenc_h264"];
    }

    public getAudioEncoders(): string[] {
        return ["aac", "mp3", "opus"];
    }

    public startFFmpeg(outputGroups: OutputGroup[]): void {
        this.logger.info("Starting FFmpeg with output groups");
    }

    public stopFFmpeg(): void {
        this.logger.info("Stopping FFmpeg processes.");
    }
}
