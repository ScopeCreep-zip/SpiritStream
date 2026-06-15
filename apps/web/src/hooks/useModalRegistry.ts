import { useCallback, useState } from 'react';

/**
 * The set of transient/object-editing modals reachable from the single-panel
 * shell. Every member must have a live caller in the same change — no
 * speculative additions. Settings/inspection DESTINATIONS (OBS, Chat, Logs, …)
 * are NOT here — they live in the unified settings window as `SettingsSection`.
 */
export type ModalName =
  | 'profileCreate'
  | 'openProfile'
  | 'targetCreate'
  | 'targetEdit'
  | 'outputGroupCreate'
  | 'outputGroupEdit'
  | 'appDrawerStream';

/**
 * Sections of the unified settings window (one window, left tab-rail). Order
 * here is the flat rail order. Opening any of these routes into the single
 * window at that section; switching sections never closes the window.
 */
export type SettingsSection =
  | 'profileEdit'
  | 'safetyWizard'
  | 'obs'
  | 'chat'
  | 'discord'
  | 'encoder'
  | 'settings'
  | 'logs'
  | 'audit'
  | 'shortcuts';

export type ModalState = Readonly<Record<ModalName, boolean>>;

const MODAL_NAMES: readonly ModalName[] = [
  'profileCreate',
  'openProfile',
  'targetCreate',
  'targetEdit',
  'outputGroupCreate',
  'outputGroupEdit',
  'appDrawerStream',
];

const INITIAL_STATE: ModalState = Object.freeze(
  MODAL_NAMES.reduce(
    (acc, name) => {
      acc[name] = false;
      return acc;
    },
    {} as Record<ModalName, boolean>
  )
);

export interface ModalRegistry {
  state: ModalState;
  open: (name: ModalName) => void;
  close: (name: ModalName) => void;
  closeAll: () => void;
  /** Active settings-window section, or null when the window is closed. */
  settingsSection: SettingsSection | null;
  /** Open the settings window at `section` (deep-link), or switch to it. */
  openSettings: (section: SettingsSection) => void;
  closeSettings: () => void;
}

export function useModalRegistry(): ModalRegistry {
  const [state, setState] = useState<ModalState>(INITIAL_STATE);
  const [settingsSection, setSettingsSection] = useState<SettingsSection | null>(null);

  const open = useCallback((name: ModalName) => {
    setState((prev) => ({ ...prev, [name]: true }));
  }, []);

  const close = useCallback((name: ModalName) => {
    setState((prev) => ({ ...prev, [name]: false }));
  }, []);

  const closeAll = useCallback(() => {
    setState(INITIAL_STATE);
    setSettingsSection(null);
  }, []);

  const openSettings = useCallback((section: SettingsSection) => {
    setSettingsSection(section);
  }, []);

  const closeSettings = useCallback(() => setSettingsSection(null), []);

  return { state, open, close, closeAll, settingsSection, openSettings, closeSettings };
}
