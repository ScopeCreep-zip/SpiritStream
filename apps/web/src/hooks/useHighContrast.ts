import { useCallback, useEffect, useState } from 'react';

const STORAGE_KEY = 'ss-high-contrast';
const HTML_ATTR = 'data-contrast';
const HTML_VALUE = 'high';

/**
 * Read the persisted high-contrast preference. Safe to call before React
 * mount — used by `applyHighContrastFromStorage` to set the attribute on
 * first paint, avoiding a FOUC for users who have it enabled.
 */
function readStoredPreference(): boolean {
  try {
    return window.localStorage.getItem(STORAGE_KEY) === '1';
  } catch {
    return false;
  }
}

function writeStoredPreference(value: boolean): void {
  try {
    if (value) {
      window.localStorage.setItem(STORAGE_KEY, '1');
    } else {
      window.localStorage.removeItem(STORAGE_KEY);
    }
  } catch {
    // localStorage unavailable (private mode, etc.) — runtime toggle still works.
  }
}

/**
 * Owns ONLY the `data-contrast` attribute on `<html>`. The `data-theme` /
 * `data-theme-id` attributes (and the theme-overrides style tag) are the
 * exclusive domain of `themeStore` — never touch them here. Theme and
 * contrast are orthogonal axes; they must not read or write each other's
 * attributes.
 */
function applyAttribute(value: boolean): void {
  const html = document.documentElement;
  if (value) {
    html.setAttribute(HTML_ATTR, HTML_VALUE);
  } else {
    html.removeAttribute(HTML_ATTR);
  }
}

/**
 * Call from `main.tsx` before render so the very first paint already
 * reflects the user's stored preference. Without this, screen readers
 * and high-contrast users would see one frame of low-contrast UI.
 */
export function applyHighContrastFromStorage(): void {
  applyAttribute(readStoredPreference());
}

export interface UseHighContrast {
  enabled: boolean;
  setEnabled: (next: boolean) => void;
  toggle: () => void;
}

export function useHighContrast(): UseHighContrast {
  const [enabled, setEnabledState] = useState<boolean>(readStoredPreference);

  // Mirror DOM ⇄ state, in case another component / hook updates the attribute.
  useEffect(() => {
    applyAttribute(enabled);
    writeStoredPreference(enabled);
  }, [enabled]);

  const setEnabled = useCallback((next: boolean) => setEnabledState(next), []);
  const toggle = useCallback(() => setEnabledState((v) => !v), []);

  return { enabled, setEnabled, toggle };
}
