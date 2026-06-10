import { useCallback, useState } from 'react';

/**
 * The set of modals reachable from the single-panel shell. Every member
 * must have a live caller in the same change — no speculative additions.
 */
export type ModalName =
  | 'profileCreate'
  | 'profileEdit'
  | 'openProfile'
  | 'targetCreate'
  | 'targetEdit'
  | 'outputGroupCreate'
  | 'outputGroupEdit'
  | 'appDrawerStream'
  | 'settings'
  | 'obs'
  | 'discord'
  | 'chat'
  | 'logs'
  | 'audit'
  | 'shortcuts'
  | 'safetyWizard';

export type ModalState = Readonly<Record<ModalName, boolean>>;

const MODAL_NAMES: readonly ModalName[] = [
  'profileCreate',
  'profileEdit',
  'openProfile',
  'targetCreate',
  'targetEdit',
  'outputGroupCreate',
  'outputGroupEdit',
  'appDrawerStream',
  'settings',
  'obs',
  'discord',
  'chat',
  'logs',
  'audit',
  'shortcuts',
  'safetyWizard',
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
}

export function useModalRegistry(): ModalRegistry {
  const [state, setState] = useState<ModalState>(INITIAL_STATE);

  const open = useCallback((name: ModalName) => {
    setState((prev) => ({ ...prev, [name]: true }));
  }, []);

  const close = useCallback((name: ModalName) => {
    setState((prev) => ({ ...prev, [name]: false }));
  }, []);

  const closeAll = useCallback(() => {
    setState(INITIAL_STATE);
  }, []);

  return { state, open, close, closeAll };
}
