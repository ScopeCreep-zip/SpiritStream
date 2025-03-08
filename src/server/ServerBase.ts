import { Profile } from '../models/Profile';
import { OutputGroup } from '../models/OutputGroup';
import { StreamTarget } from '../models/StreamTarget';

export abstract class ServerBase {

    //Server start, stop, and test
    public abstract start(): void;
    public abstract stop(): void;
    public abstract test(): void;

    // ROUTES
    // Profiles
    public abstract getProfiles(): Profile[];
    public abstract getProfile(id: string): Profile;
    public abstract createProfile(profile: Profile): void;
    public abstract updateProfile(profile: Profile): void;
    public abstract deleteProfile(id: string): void;

    // Output Groups
    public abstract getOutputGroups(): OutputGroup[];
    public abstract getOutputGroup(id: string): OutputGroup;
    public abstract createOutputGroup(outputGroup: OutputGroup): void;
    public abstract updateOutputGroup(outputGroup: OutputGroup): void;
    public abstract deleteOutputGroup(id: string): void;

    // Stream Targets
    public abstract getStreamTargets(): StreamTarget[];
    public abstract getStreamTarget(id: string): StreamTarget;
    public abstract createStreamTarget(streamTarget: StreamTarget): void;
    public abstract updateStreamTarget(streamTarget: StreamTarget): void;

    // FFmpeg
    public abstract testFFmpeg(): string;
    public abstract getVideoEncoders(): string[];
    public abstract getAudioEncoders(): string[];
    public abstract startFFmpeg(outputGroups: OutputGroup[]): void;
    public abstract stopFFmpeg(): void;
}

