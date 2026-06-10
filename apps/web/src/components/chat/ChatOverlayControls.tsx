import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { SquareArrowOutUpRight, SquareArrowDownLeft } from 'lucide-react';
import { emit } from '@tauri-apps/api/event';
import { Button } from '@/components/ui/Button';
import { Toggle } from '@/components/ui/Toggle';
import { CHAT_OVERLAY_SETTINGS_EVENT, CHAT_OVERLAY_ALWAYS_ON_TOP_EVENT } from '@/lib/chatEvents';
import { closeChatOverlay, openChatOverlay, setOverlayAlwaysOnTop } from '@/lib/chatWindow';
import { useChatStore } from '@/stores/chatStore';
import { logger } from '@/lib/logger';

/**
 * Pop-out / Dock icon buttons (rendered in the CardHeader alongside the
 * "Unified Chat" title) + transparency / always-on-top toggles
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
  const overlayTransparent = useChatStore((state) => state.overlayTransparent);
  const setOverlayTransparent = useChatStore((state) => state.setOverlayTransparent);
  const overlayAlwaysOnTop = useChatStore((state) => state.overlayAlwaysOnTop);
  const setOverlayAlwaysOnTopState = useChatStore((state) => state.setOverlayAlwaysOnTop);

  const handleTransparentToggle = useCallback(
    (transparent: boolean) => {
      setOverlayTransparent(transparent);
      emit(CHAT_OVERLAY_SETTINGS_EVENT, { transparent }).catch((error) => {
        logger.error('Failed to sync chat overlay settings:', error);
      });
    },
    [setOverlayTransparent]
  );

  const handleAlwaysOnTopToggle = useCallback(
    (alwaysOnTop: boolean) => {
      setOverlayAlwaysOnTopState(alwaysOnTop);
      setOverlayAlwaysOnTop(alwaysOnTop);
      emit(CHAT_OVERLAY_ALWAYS_ON_TOP_EVENT, { alwaysOnTop }).catch((error) => {
        logger.error('Failed to sync chat overlay always on top:', error);
      });
    },
    [setOverlayAlwaysOnTopState]
  );

  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
      <Toggle
        checked={overlayTransparent}
        onChange={handleTransparentToggle}
        label={t('chat.transparentOverlay', { defaultValue: 'Transparent overlay' })}
      />
      <Toggle
        checked={overlayAlwaysOnTop}
        onChange={handleAlwaysOnTopToggle}
        label={t('chat.alwaysOnTop', { defaultValue: 'Always on top' })}
      />
    </div>
  );
}
