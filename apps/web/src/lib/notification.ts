import { isTauri } from '@/lib/platform';
import { sendNotification } from '@tauri-apps/plugin-notification';

export async function showSystemNotification(title: string, body: string) {
  if (!isTauri) return;
  try {
    await sendNotification({ title, body });
  } catch (err) {
    // eslint-disable-next-line no-console
    console.warn('System notification failed:', err);
  }
}
