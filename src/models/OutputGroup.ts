import { StreamTarget } from "./StreamTarget";

export class OutputGroup {
  private id: string;
  private name: string;
  private videoEncoder: string;
  private resolution: string;
  private bitrate: string;
  private fps: string;
  private audioCodec: string;
  private audioBitrate: string;
  private generatePTS: boolean;
  private streamTargets: StreamTarget[];


  constructor(
    id: string,
    name: string,
    videoEncoder: string,
    resolution: string,
    bitrate: string,
    fps: string,
    audioCodec: string,
    audioBitrate: string,
    generatePTS: boolean
  ) {
    this.id = id;
    this.name = name;
    this.videoEncoder = videoEncoder;
    this.resolution = resolution;
    this.bitrate = bitrate;
    this.fps = fps;
    this.audioCodec = audioCodec;
    this.audioBitrate = audioBitrate;
    this.generatePTS = generatePTS;
    this.streamTargets = [];
  }

  // Getters
  public getId(): string {
    return this.id;
  }

  public getName(): string {
    return this.name;
  }

  public getVideoEncoder(): string {
    return this.videoEncoder;
  }

  public getResolution(): string {
    return this.resolution;
  }

  public getBitrate(): string {
    return this.bitrate;
  }

  public getFps(): string {
    return this.fps;
  }

  public getAudioCodec(): string {
    return this.audioCodec;
  }

  public getAudioBitrate(): string {
    return this.audioBitrate;
  }

  public isPTSGenerated(): boolean {
    return this.generatePTS;
  }

  public getStreamTargets(): StreamTarget[] {
    return this.streamTargets;
  }

  // Setters
  public setName(newName: string): void {
    this.name = newName;
  }

  public setVideoEncoder(encoder: string): void {
    this.videoEncoder = encoder;
  }

  public setResolution(resolution: string): void {
    this.resolution = resolution;
  }

  public setBitrate(bitrate: string): void {
    this.bitrate = bitrate;
  }

  public setFps(fps: string): void {
    this.fps = fps;
  }

  public setAudioCodec(codec: string): void {
    this.audioCodec = codec;
  }

  public setAudioBitrate(bitrate: string): void {
    this.audioBitrate = bitrate;
  }

  public setPTSGeneration(value: boolean): void {
    this.generatePTS = value;
  }

  // Stream Target Management
  public addStreamTarget(target: StreamTarget): void {
    this.streamTargets.push(target);
  }

  public removeStreamTarget(targetId: string): void {
    this.streamTargets = this.streamTargets.filter(t => t.getId() !== targetId);
  }

  public getStreamTargetById(targetId: string): StreamTarget | undefined {
    return this.streamTargets.find(t => t.getId() === targetId);
  }

  // Export OutputGroup as JSON
  public export(): string {
    return JSON.stringify({
      id: this.id,
      name: this.name,
      videoEncoder: this.videoEncoder,
      resolution: this.resolution,
      bitrate: this.bitrate,
      fps: this.fps,
      audioCodec: this.audioCodec,
      audioBitrate: this.audioBitrate,
      generatePTS: this.generatePTS,
      streamTargets: this.streamTargets.map(target => target.export()),
    }, null, 2);
  }
}