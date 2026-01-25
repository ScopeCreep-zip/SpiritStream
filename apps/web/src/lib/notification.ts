import { isTauri } from '@/lib/platform';
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification';

let permissionChecked = false;
let permissionGranted = false;

export async function showSystemNotification(title: string, body: string) {
  // Debug: log what's available on window
  console.log('[notification] window.__TAURI__:', window.__TAURI__);
  console.log('[notification] window.__TAURI_INTERNALS__:', (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__);

  const inTauri = isTauri();
  console.log('[notification] showSystemNotification called:', { title, body, isTauri: inTauri });

  if (!inTauri) {
    console.log('[notification] Not in Tauri, skipping');
    return;
  }

  try {
    // Check/request permission on first call
    if (!permissionChecked) {
      permissionGranted = await isPermissionGranted();
      console.log('[notification] isPermissionGranted:', permissionGranted);
      if (!permissionGranted) {
        const permission = await requestPermission();
        console.log('[notification] requestPermission result:', permission);
        permissionGranted = permission === 'granted';
      }
      permissionChecked = true;
    }

    if (!permissionGranted) {
      console.log('[notification] Permission not granted, skipping');
      return;
    }

    console.log('[notification] Sending notification...');
    await sendNotification({ title, body });
    console.log('[notification] Notification sent successfully');
  } catch (err) {
    // eslint-disable-next-line no-console
    console.warn('System notification failed:', err);
  }
}
