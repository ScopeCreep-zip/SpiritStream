export class StreamTarget {
  private id: string;
  private url: string;
  private streamKey: string;
  private rtmpPort: number;

  constructor(id: string, url: string, streamKey: string, rtmpPort: number = 1935) {
    this.id = id;
    this.url = url.trim().replace(/\/+$/, '');  // Strip trailing slashes and trim whitespace
    this.streamKey = streamKey.trim();
    this.rtmpPort = rtmpPort;
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

  public getPort(): number {
    return this.rtmpPort;
  }

  // Setters
  public setUrl(newUrl: string): void {
    this.url = newUrl;
  }

  public setStreamKey(newKey: string): void {
    this.streamKey = newKey;
  }

  public setPort(newPort: number): void {
    this.rtmpPort = newPort;
  }

  // Computed property to return the normalized URL
  public get normalizedPath(): string {
    let targetUrl = this.url;

    // Ensure the URL starts with rtmp://
    if (!targetUrl.startsWith("rtmp://")) {
      targetUrl = `rtmp://${targetUrl}`;
    }

    // Parse the URL to ensure it has the domain and the rest of the path
    const parsedUrl = new URL(targetUrl);

    // Strip common placeholders and ensure proper formatting
    const domain = parsedUrl.hostname;
    const path = parsedUrl.pathname.replace(/\/$/, ''); // Remove trailing slash if any
    const port = parsedUrl.port || this.rtmpPort; // Use provided port or default to 1935
    const streamKey = this.streamKey;

    // Return the properly formatted stream URL
    return `${parsedUrl.protocol}//${domain}:${port}${path}/${streamKey}`;
  }

  // Export StreamTarget as JSON, now including the normalized URL
  public export(): string {
    return JSON.stringify({
      id: this.id,
      url: this.url,
      streamKey: this.streamKey,
      rtmpPort: this.rtmpPort,
      normalizedPath: this.normalizedPath, // Include the normalized path in the export
    }, null, 2);
  }
}
