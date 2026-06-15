import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { SquareArrowOutUpRight, SquareArrowDownLeft } from 'lucide-react';
import { emit } from '@tauri-apps/api/event';
import { Button } from '@/components/ui/Button';
import { Toggle } from '@/components/ui/Toggle';
import { CHAT_OVERLAY_SETTINGS_EVENT, CHAT_OVERLAY_ALWAYS_ON_TOP_EVENT } from '@/lib/chatEvents';
import { closeChatOverlay, openChatOverlay, setOverlayAlwaysOnTop } from '@/lib/chatWindow';
import { useChatStore } from '@/stores/chatStore';
import { isTauri } from '@spiritstream/api-client';
import { logger } from '@/lib/logger';

/**
 * Pop-out / Dock icon buttons (rendered in the CardHeader alongside the
 * "Unified Chat" title) + the overlay-style selector / always-on-top toggle
 * (rendered in the body).
 *
 * Both surfaces talk to the same overlay window via `chatWindow` + Tauri
 * events; bundled in one file because they're conceptually one feature
 * (managing the pop-out overlay).
 */

interface ChatOverlayHeaderButtonsProps {}

export function ChatOverlayHeaderButtons(
  _props: ChatOverlayHeaderButtonsProps = {}
): React.ReactElement {
  const { t } = useTranslation();
  return (
    <div className="flex items-center gap-1 flex-shrink-0">
      <Button
        size="icon"
        onClick={openChatOverlay}
        aria-label={t('chat.popOut', { defaultValue: 'Pop out' })}
        title={t('chat.popOut', { defaultValue: 'Pop out' })}
      >
        <SquareArrowOutUpRight className="w-4 h-4" />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        onClick={closeChatOverlay}
        aria-label={t('chat.dockChat', { defaultValue: 'Dock chat' })}
        title={t('chat.dockChat', { defaultValue: 'Dock chat' })}
      >
        <SquareArrowDownLeft className="w-4 h-4" />
      </Button>
    </div>
  );
}

export function ChatOverlayToggles(): React.ReactElement {
  const { t } = useTranslation();
  const overlayOpacity = useChatStore((state) => state.overlayOpacity);
  const setOverlayOpacity = useChatStore((state) => state.setOverlayOpacity);
  const overlayAlwaysOnTop = useChatStore((state) => state.overlayAlwaysOnTop);
  const setOverlayAlwaysOnTopState = useChatStore((state) => state.setOverlayAlwaysOnTop);

  // Cross-window sync of the overlay opacity (Tauri-only).
  const emitOverlayOpacity = useCallback((opacity: number) => {
    if (isTauri()) {
      emit(CHAT_OVERLAY_SETTINGS_EVENT, { opacity }).catch((error) => {
        logger.error('Failed to sync chat overlay settings:', error);
      });
    }
  }, []);

  // Slider is oriented as TRANSPARENCY (right = more see-through): opacity =
  // 1 - transparency, so transparency 1 → fully clear, 0 → opaque.
  const transparency = 1 - overlayOpacity;
  const handleTransparencyChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const opacity = Math.round((1 - Number(e.target.value)) * 100) / 100;
      setOverlayOpacity(opacity);
      emitOverlayOpacity(opacity);
    },
    [setOverlayOpacity, emitOverlayOpacity]
  );

  const handleAlwaysOnTopToggle = useCallback(
    (alwaysOnTop: boolean) => {
      setOverlayAlwaysOnTopState(alwaysOnTop);
      setOverlayAlwaysOnTop(alwaysOnTop);
      if (isTauri()) {
        emit(CHAT_OVERLAY_ALWAYS_ON_TOP_EVENT, { alwaysOnTop }).catch((error) => {
          logger.error('Failed to sync chat overlay always on top:', error);
        });
      }
    },
    [setOverlayAlwaysOnTopState]
  );

  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
      <label className="flex items-center gap-2 text-sm">
        <span className="text-text-secondary">
          {t('chat.overlay.transparency', { defaultValue: 'Transparency' })}
        </span>
        <input
          type="range"
          min={0}
          max={1}
          step={0.05}
          value={transparency}
          onChange={handleTransparencyChange}
          aria-label={t('chat.overlay.transparency', { defaultValue: 'Transparency' })}
          className="w-28 cursor-pointer [accent-color:var(--primary)]"
        />
      </label>
      <Toggle
        checked={overlayAlwaysOnTop}
        onChange={handleAlwaysOnTopToggle}
        label={t('chat.alwaysOnTop', { defaultValue: 'Always on top' })}
      />
    </div>
  );
}
