import { useCallback, useEffect, useState } from 'react';
import type { HotkeyBinding } from '@/lib/hotkey';

/**
 * Persisted user-configurable hotkey, mirrored across browser tabs via the
 * native `storage` event. One binding per name (`'panic'`, future others).
 */
export type HotkeyName = 'panic';

const STORAGE_PREFIX = 'ss-hotkey-';

function storageKey(name: HotkeyName): string {
  return `${STORAGE_PREFIX}${name}`;
}

function readBinding(name: HotkeyName, fallback: HotkeyBinding): HotkeyBinding {
  try {
    const raw = window.localStorage.getItem(storageKey(name));
    if (!raw) return fallback;
    const parsed = JSON.parse(raw) as unknown;
    if (
      parsed &&
      typeof parsed === 'object' &&
      'mods' in parsed &&
      Array.isArray((parsed as HotkeyBinding).mods) &&
      'key' in parsed &&
      typeof (parsed as HotkeyBinding).key === 'string'
    ) {
      return parsed as HotkeyBinding;
    }
    return fallback;
  } catch {
    return fallback;
  }
}

function writeBinding(name: HotkeyName, binding: HotkeyBinding): void {
  try {
    window.localStorage.setItem(storageKey(name), JSON.stringify(binding));
  } catch {
    // localStorage unavailable — runtime binding still works for this session.
  }
}

export interface UseStoredHotkey {
  binding: HotkeyBinding;
  setBinding: (next: HotkeyBinding) => void;
}

export function useStoredHotkey(name: HotkeyName, defaultBinding: HotkeyBinding): UseStoredHotkey {
  const [binding, setBindingState] = useState<HotkeyBinding>(() =>
    readBinding(name, defaultBinding)
  );

  // Listen for cross-tab updates (another window of the same app may rebind).
  useEffect(() => {
    const handler = (e: StorageEvent): void => {
      if (e.key !== storageKey(name)) return;
      setBindingState(readBinding(name, defaultBinding));
    };
    window.addEventListener('storage', handler);
    return () => window.removeEventListener('storage', handler);
  }, [name, defaultBinding]);

  const setBinding = useCallback(
    (next: HotkeyBinding): void => {
      writeBinding(name, next);
      setBindingState(next);
    },
    [name]
  );

  return { binding, setBinding };
}
