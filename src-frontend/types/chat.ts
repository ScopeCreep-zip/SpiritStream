import type { Platform } from './profile';

export interface ChatMessage {
  id: string;
  platform: Platform;
  username: string;
  message: string;
  timestamp: number;
}
