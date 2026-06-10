import { useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { register, unregister } from '@tauri-apps/plugin-global-shortcut';
import { isTauri } from '@spiritstream/api-client';
import { api } from '@/lib/client';
import { toast } from '@/hooks/useToast';
import { logger } from '@/lib/logger';
import { useStoredHotkey } from '@/hooks/useStoredHotkey';
import { matchesEvent, toTauriShortcut, type HotkeyBinding } from '@/lib/hotkey';

/**
 * Default panic binding — `⌘P` on macOS, `Ctrl+P` on Windows/Linux. The
 * `cmdOrCtrl` modifier collapses both into a single cross-platform binding
 * so the user's choice travels with them.
 */
export const DEFAULT_PANIC_BINDING: HotkeyBinding = {
  mods: ['cmdOrCtrl'],
  key: 'p',
};

/**
 * Wire the panic hotkey.
 *
 * - In a Tauri webview: registers a system-global shortcut via
 *   `tauri-plugin-global-shortcut`. Fires even when SpiritStream isn't the
 *   focused application — the whole point of a panic key for a streaming
 *   app where a creator may have other windows on top.
 * - In a browser: falls back to a window-level keydown handler. Only fires
 *   when the SpiritStream tab has focus, but that's the platform's ceiling.
 *
 * Binding is read from `useStoredHotkey('panic', DEFAULT_PANIC_BINDING)`;
 * rebinding via the Accessibility settings re-runs the effect (cleanup +
 * register) automatically.
 */
export function usePanicHotkey(): void {
  const { t } = useTranslation();
  const { binding } = useStoredHotkey('panic', DEFAULT_PANIC_BINDING);

  const handlePanic = useCallback(async (): Promise<void> => {
    try {
      const result = await api.safety.panic();
      toast.success(
        t('toast.panicStopped', {
          count: result.streamsStopped,
          ms: result.elapsedMs,
          defaultValue: 'Panic disconnect: stopped {{count}} streams in {{ms}}ms',
        })
      );
    } catch (err) {
      logger.error('[panic] failed', err);
      toast.error(
        t('toast.panicFailed', {
          defaultValue: 'Panic disconnect failed: {{error}}',
          error: err instanceof Error ? err.message : String(err),
        })
      );
    }
  }, [t]);

  useEffect(() => {
    if (isTauri()) {
      const shortcut = toTauriShortcut(binding);
      let cancelled = false;
      register(shortcut, (event) => {
        // Plugin fires for both keydown and keyup; only act on keydown
        // to avoid firing twice per press.
        if (event.state === 'Pressed') void handlePanic();
      }).catch((err) => {
        logger.error('[panic] global-shortcut register failed', err);
      });
      return () => {
        cancelled = true;
        unregister(shortcut).catch((err) => {
          if (!cancelled) logger.error('[panic] global-shortcut unregister failed', err);
        });
      };
    }

    // Web fallback — window-level handler only fires when the tab has focus.
    const handler = (e: KeyboardEvent): void => {
      if (!matchesEvent(binding, e)) return;
      e.preventDefault();
      e.stopPropagation();
      void handlePanic();
    };
    window.addEventListener('keydown', handler, { capture: true });
    return () => window.removeEventListener('keydown', handler, { capture: true });
  }, [binding, handlePanic]);
}
