import { listen } from '@tauri-apps/api/event';

type Handler<T> = (payload: T) => void;

export const events = {
  on: <T>(eventName: string, handler: Handler<T>) =>
    listen<T>(eventName, (event) => handler(event.payload)),
};
