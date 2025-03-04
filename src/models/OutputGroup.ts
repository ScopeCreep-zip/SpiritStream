// OutputGroup.ts

import { StreamTarget } from "./StreamTarget";

export class OutputGroup {
  id: string;
  name: string;
  videoEncoder: string;
  resolution: string;
  bitrate: string;
  fps: string;
  audioCodec: string;
  audioBitrate: string;
  streamTargets: StreamTarget[];

  constructor(
    id: string,
    name: string,
    videoEncoder: string,
    resolution: string,
    bitrate: string,
    fps: string,
    audioCodec: string,
    audioBitrate: string
  ) {
    this.id = id;
    this.name = name;
    this.videoEncoder = videoEncoder;
    this.resolution = resolution;
    this.bitrate = bitrate;
    this.fps = fps;
    this.audioCodec = audioCodec;
    this.audioBitrate = audioBitrate;
    this.streamTargets = [];
  }

  // Add a stream target to the group
  addStreamTarget(target: StreamTarget) {
    this.streamTargets.push(target);
  }

  // Remove a stream target by ID
  removeStreamTarget(targetId: string) {
    this.streamTargets = this.streamTargets.filter((t) => t.id !== targetId);
  }
}
