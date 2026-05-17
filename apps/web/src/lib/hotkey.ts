/**
 * Cross-platform hotkey representation + serialization.
 *
 * Modifier set is intentionally narrow: `cmdOrCtrl` (the platform-native
 * primary mod), `alt`, `shift`. This collapses macOS `meta` and Win/Linux
 * `ctrl` into a single binding that works across all of them — the user
 * binds once and it travels with their profile.
 */

export type HotkeyMod = 'cmdOrCtrl' | 'alt' | 'shift';

export interface HotkeyBinding {
  readonly mods: ReadonlyArray<HotkeyMod>;
  /** Single key name, lowercased (e.g. 'p', 'enter', '/'). */
  readonly key: string;
}

const MOD_ORDER: ReadonlyArray<HotkeyMod> = ['cmdOrCtrl', 'alt', 'shift'];

function sortMods(mods: ReadonlyArray<HotkeyMod>): ReadonlyArray<HotkeyMod> {
  return MOD_ORDER.filter((m) => mods.includes(m));
}

/**
 * Convert a binding to Tauri global-shortcut format.
 * See: https://docs.rs/tauri-plugin-global-shortcut/2/
 */
export function toTauriShortcut(b: HotkeyBinding): string {
  const parts: string[] = [];
  for (const m of sortMods(b.mods)) {
    if (m === 'cmdOrCtrl') parts.push('CommandOrControl');
    else if (m === 'alt') parts.push('Alt');
    else if (m === 'shift') parts.push('Shift');
  }
  parts.push(b.key.length === 1 ? b.key.toUpperCase() : b.key);
  return parts.join('+');
}

/**
 * Check whether a browser `KeyboardEvent` matches this binding exactly.
 * Both required mods must be down AND no foreign mods. Key match is
 * case-insensitive.
 */
export function matchesEvent(b: HotkeyBinding, e: KeyboardEvent): boolean {
  const wantsCmdOrCtrl = b.mods.includes('cmdOrCtrl');
  const hasCmdOrCtrl = e.metaKey || e.ctrlKey;
  if (wantsCmdOrCtrl !== hasCmdOrCtrl) return false;
  if (b.mods.includes('alt') !== e.altKey) return false;
  if (b.mods.includes('shift') !== e.shiftKey) return false;
  return e.key.toLowerCase() === b.key.toLowerCase();
}

/**
 * Render the binding as a list of display tokens. Platform-aware so macOS
 * users see `⌘ ⌥ ⇧` and Windows/Linux users see `Ctrl Alt Shift`.
 */
export function formatBindingTokens(b: HotkeyBinding, isMac: boolean): string[] {
  const tokens: string[] = [];
  for (const m of sortMods(b.mods)) {
    if (m === 'cmdOrCtrl') tokens.push(isMac ? '⌘' : 'Ctrl');
    else if (m === 'alt') tokens.push(isMac ? '⌥' : 'Alt');
    else if (m === 'shift') tokens.push(isMac ? '⇧' : 'Shift');
  }
  tokens.push(b.key.length === 1 ? b.key.toUpperCase() : b.key);
  return tokens;
}

export function isMacPlatform(): boolean {
  if (typeof navigator === 'undefined') return false;
  return /Mac|iPhone|iPad|iPod/.test(navigator.platform);
}

/**
 * Build a binding from a `KeyboardEvent` captured while the user is
 * setting a new hotkey. Returns `null` if the event is a modifier-only
 * press (caller should ignore and wait for the next event).
 */
export function eventToBinding(e: KeyboardEvent): HotkeyBinding | null {
  const key = e.key;
  // Bare modifier press — wait for a real key.
  if (
    key === 'Meta' ||
    key === 'Control' ||
    key === 'Alt' ||
    key === 'Shift' ||
    key === 'OS' ||
    key === 'Hyper'
  ) {
    return null;
  }
  const mods: HotkeyMod[] = [];
  if (e.metaKey || e.ctrlKey) mods.push('cmdOrCtrl');
  if (e.altKey) mods.push('alt');
  if (e.shiftKey) mods.push('shift');
  return { mods, key: key.toLowerCase() };
}
