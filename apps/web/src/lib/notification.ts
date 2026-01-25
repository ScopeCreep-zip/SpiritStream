import { isTauri } from '@/lib/platform';
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification';

let permissionChecked = false;
let permissionGranted = false;

export async function showSystemNotification(title: string, body: string) {
  if (!isTauri()) return;

  try {
    // Check/request permission on first call
    if (!permissionChecked) {
      permissionGranted = await isPermissionGranted();
      if (!permissionGranted) {
        const permission = await requestPermission();
        permissionGranted = permission === 'granted';
      }
      permissionChecked = true;
    }

    if (!permissionGranted) return;

    await sendNotification({ title, body });
  } catch (err) {
    console.warn('System notification failed:', err);
  }
}
