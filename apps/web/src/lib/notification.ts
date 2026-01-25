import { isTauri } from '@/lib/platform';

export async function showSystemNotification(title: string, body: string) {
  if (!isTauri) return;
  try {
    // Dynamically import to avoid breaking web build
    const { sendNotification } = await import('@tauri-apps/api/notification');
    await sendNotification({ title, body });
  } catch (err) {
    // eslint-disable-next-line no-console
    console.warn('System notification failed:', err);
  }
}
