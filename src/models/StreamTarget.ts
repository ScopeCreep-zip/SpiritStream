export class StreamTarget {
  private id: string;
  private url: string;
  private streamKey: string;

  constructor(id: string, url: string, streamKey: string) {
    this.id = id;
    this.url = url;
    this.streamKey = streamKey;
  }

  // Getters
  public getId(): string {
    return this.id;
  }

  public getUrl(): string {
    return this.url;
  }

  public getStreamKey(): string {
    return this.streamKey;
  }

  // Setters
  public setUrl(newUrl: string): void {
    this.url = newUrl;
  }

  public setStreamKey(newKey: string): void {
    this.streamKey = newKey;
  }

  // Export StreamTarget as JSON
  public export(): string {
    return JSON.stringify({
      id: this.id,
      url: this.url,
      streamKey: this.streamKey,
    }, null, 2);
  }
}