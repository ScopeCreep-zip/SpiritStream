// StreamTarget.ts

export class StreamTarget {
    id: string;
    url: string;
    streamKey: string;
  
    constructor(id: string, url: string, streamKey: string) {
      this.id = id;
      this.url = url;
      this.streamKey = streamKey;
    }
  }
  