/**
 * Hotkey types for global keyboard shortcuts
 */

/**
 * Available hotkey actions
 */
export type HotkeyAction =
  | 'take'
  | 'scene1'
  | 'scene2'
  | 'scene3'
  | 'scene4'
  | 'scene5'
  | 'scene6'
  | 'scene7'
  | 'scene8'
  | 'scene9'
  | 'toggleStudioMode'
  | 'escape'
  | 'toggleMute'
  | 'toggleStream';

/**
 * Hotkey binding configuration
 */
export interface HotkeyBinding {
  id: string;
  action: HotkeyAction;
  /** Keyboard event key code (e.g., 'Space', 'Digit1', 'KeyS') */
  key: string;
  /** Display label for the key (e.g., 'Space', '1', 'S') */
  displayKey: string;
  modifiers: {
    ctrl?: boolean;
    alt?: boolean;
    shift?: boolean;
    meta?: boolean;
  };
  enabled: boolean;
}

/**
 * Default hotkey bindings
 */
export const DEFAULT_BINDINGS: HotkeyBinding[] = [
  // Take (transition) bindings
  {
    id: 'take-space',
    action: 'take',
    key: 'Space',
    displayKey: 'Space',
    modifiers: {},
    enabled: true,
  },
  {
    id: 'take-enter',
    action: 'take',
    key: 'Enter',
    displayKey: 'Enter',
    modifiers: {},
    enabled: true,
  },

  // Scene switching (1-9)
  {
    id: 'scene-1',
    action: 'scene1',
    key: 'Digit1',
    displayKey: '1',
    modifiers: {},
    enabled: true,
  },
  {
    id: 'scene-2',
    action: 'scene2',
    key: 'Digit2',
    displayKey: '2',
    modifiers: {},
    enabled: true,
  },
  {
    id: 'scene-3',
    action: 'scene3',
    key: 'Digit3',
    displayKey: '3',
    modifiers: {},
    enabled: true,
  },
  {
    id: 'scene-4',
    action: 'scene4',
    key: 'Digit4',
    displayKey: '4',
    modifiers: {},
    enabled: true,
  },
  {
    id: 'scene-5',
    action: 'scene5',
    key: 'Digit5',
    displayKey: '5',
    modifiers: {},
    enabled: true,
  },
  {
    id: 'scene-6',
    action: 'scene6',
    key: 'Digit6',
    displayKey: '6',
    modifiers: {},
    enabled: true,
  },
  {
    id: 'scene-7',
    action: 'scene7',
    key: 'Digit7',
    displayKey: '7',
    modifiers: {},
    enabled: true,
  },
  {
    id: 'scene-8',
    action: 'scene8',
    key: 'Digit8',
    displayKey: '8',
    modifiers: {},
    enabled: true,
  },
  {
    id: 'scene-9',
    action: 'scene9',
    key: 'Digit9',
    displayKey: '9',
    modifiers: {},
    enabled: true,
  },

  // Escape to deselect
  {
    id: 'escape',
    action: 'escape',
    key: 'Escape',
    displayKey: 'Esc',
    modifiers: {},
    enabled: true,
  },

  // Toggle Studio Mode
  {
    id: 'studio',
    action: 'toggleStudioMode',
    key: 'KeyS',
    displayKey: 'S',
    modifiers: { ctrl: true },
    enabled: true,
  },
];

/**
 * Get human-readable label for hotkey action
 */
export function getActionLabel(action: HotkeyAction): string {
  switch (action) {
    case 'take':
      return 'Take (Transition)';
    case 'scene1':
      return 'Scene 1';
    case 'scene2':
      return 'Scene 2';
    case 'scene3':
      return 'Scene 3';
    case 'scene4':
      return 'Scene 4';
    case 'scene5':
      return 'Scene 5';
    case 'scene6':
      return 'Scene 6';
    case 'scene7':
      return 'Scene 7';
    case 'scene8':
      return 'Scene 8';
    case 'scene9':
      return 'Scene 9';
    case 'toggleStudioMode':
      return 'Toggle Studio Mode';
    case 'escape':
      return 'Deselect / Cancel';
    case 'toggleMute':
      return 'Toggle Mute';
    case 'toggleStream':
      return 'Toggle Stream';
    default:
      return action;
  }
}

/**
 * Format a hotkey binding for display
 */
export function formatHotkeyBinding(binding: HotkeyBinding): string {
  const parts: string[] = [];

  if (binding.modifiers.ctrl) parts.push('Ctrl');
  if (binding.modifiers.alt) parts.push('Alt');
  if (binding.modifiers.shift) parts.push('Shift');
  if (binding.modifiers.meta) parts.push('Cmd');

  parts.push(binding.displayKey);

  return parts.join('+');
}
