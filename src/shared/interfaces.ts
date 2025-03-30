export interface StreamTargetDTO {
    id: string;
    url: string;
    streamKey: string;
    rtmpPort: number;
    normalizedPath: string;
  }
  
  export interface OutputGroupDTO {
    id: string;
    name: string;
    videoEncoder: string;
    resolution: string;
    bitrate: string;
    fps: string;
    audioCodec: string;
    audioBitrate: string;
    generatePTS: boolean;
    streamTargets: StreamTargetDTO[];
  }
  
  export interface ThemeDTO {
    id: string;
    name: string;
    primaryColor: string;
    secondaryColor: string;
    backgroundColor: string;
    textColor: string;
    darkMode: boolean;
  }
  
  export interface ProfileDTO {
    id: string;
    name: string;
    incomingURL: string;
    outputGroups: OutputGroupDTO[];
    theme?: ThemeDTO;
  }
  