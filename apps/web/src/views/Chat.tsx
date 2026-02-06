import { useTranslation } from 'react-i18next';
import { SquareArrowOutUpRight, Trash2 } from 'lucide-react';
import { emit } from '@tauri-apps/api/event';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Toggle } from '@/components/ui/Toggle';
import { ChatList } from '@/components/chat/ChatList';
import { CHAT_OVERLAY_SETTINGS_EVENT, CHAT_OVERLAY_ALWAYS_ON_TOP_EVENT } from '@/lib/chatEvents';
import { openChatOverlay, setOverlayAlwaysOnTop } from '@/lib/chatWindow';
import { useChatStore } from '@/stores/chatStore';

export function Chat() {
  const { t } = useTranslation();
  const messages = useChatStore((state) => state.messages);
  const overlayTransparent = useChatStore((state) => state.overlayTransparent);
  const setOverlayTransparent = useChatStore((state) => state.setOverlayTransparent);
  const overlayAlwaysOnTop = useChatStore((state) => state.overlayAlwaysOnTop);
  const setOverlayAlwaysOnTopState = useChatStore((state) => state.setOverlayAlwaysOnTop);
  const clearMessages = useChatStore((state) => state.clearMessages);

  const handleTransparentToggle = (transparent: boolean) => {
    setOverlayTransparent(transparent);
    emit(CHAT_OVERLAY_SETTINGS_EVENT, { transparent }).catch((error) => {
      console.error('Failed to sync chat overlay settings:', error);
    });
  };

  const handleAlwaysOnTopToggle = (alwaysOnTop: boolean) => {
    setOverlayAlwaysOnTopState(alwaysOnTop);
    setOverlayAlwaysOnTop(alwaysOnTop);
    emit(CHAT_OVERLAY_ALWAYS_ON_TOP_EVENT, { alwaysOnTop }).catch((error) => {
      console.error('Failed to sync chat overlay always on top:', error);
    });
  };

  return (
    <Card>
      <CardHeader>
        <div>
          <CardTitle>{t('chat.title', { defaultValue: 'Unified Chat' })}</CardTitle>
          <CardDescription>
            {t('chat.description', {
              defaultValue: 'Read-only chat messages from your connected platforms.',
            })}
          </CardDescription>
        </div>
      </CardHeader>
      <CardBody>
        <div className="flex flex-wrap items-center justify-between" style={{ gap: '16px' }}>
          <div className="flex flex-wrap items-center" style={{ gap: '24px' }}>
            <Toggle
              checked={overlayTransparent}
              onChange={handleTransparentToggle}
              label={t('chat.transparentOverlay', { defaultValue: 'Transparent overlay' })}
              description={t('chat.transparentOverlayDescription', {
                defaultValue: 'Makes the pop-out window background transparent.',
              })}
            />
            <Toggle
              checked={overlayAlwaysOnTop}
              onChange={handleAlwaysOnTopToggle}
              label={t('chat.alwaysOnTop', { defaultValue: 'Always on top' })}
              description={t('chat.alwaysOnTopDescription', {
                defaultValue: 'Keeps the pop-out window above other windows.',
              })}
            />
          </div>
          <div className="flex items-center" style={{ gap: '12px' }}>
            <Button variant="ghost" size="sm" onClick={clearMessages}>
              <Trash2 className="w-4 h-4" />
              {t('common.clear')}
            </Button>
            <Button size="sm" onClick={openChatOverlay}>
              <SquareArrowOutUpRight className="w-4 h-4" />
              {t('chat.popOut', { defaultValue: 'Pop out' })}
            </Button>
          </div>
        </div>
        <div className="mt-6">
          <ChatList
            messages={messages}
            className="max-h-[520px]"
            emptyLabel={t('chat.empty', { defaultValue: 'No chat messages yet.' })}
          />
        </div>
      </CardBody>
    </Card>
  );
}
