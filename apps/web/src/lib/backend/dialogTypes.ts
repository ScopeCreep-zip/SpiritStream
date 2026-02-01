export interface DialogFilter {
  name: string;
  extensions: string[];
}

export interface OpenFileOptions {
  multiple?: boolean;
  directory?: boolean;
  filters?: DialogFilter[];
  title?: string;
}

export interface SaveFileOptions {
  defaultPath?: string;
  title?: string;
  filters?: DialogFilter[];
}

export interface OpenTextResult {
  name: string;
  content: string;
}
