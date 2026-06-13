import { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Trash2, Download, Search } from 'lucide-react';
import { Card, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { ChatList } from '@/components/chat/ChatList';
import { useChatStore } from '@/stores/chatStore';
import { api } from '@/lib/client';
import { useFileBrowser } from '@/hooks/useFileBrowser';
import { useChatPlatformStatus } from '@/hooks/useChatPlatformStatus';
import {
  ChatOverlayHeaderButtons,
  ChatOverlayToggles,
} from '@/components/chat/ChatOverlayControls';
import { ChatComposer } from '@/components/chat/ChatComposer';
import { ChatReauthBanner } from '@/components/chat/ChatReauthBanner';
import { ChannelModeBanner } from '@/components/chat/ChannelModeBanner';
import { ChatSearch } from '@/components/chat/ChatSearch';
import { useRoomStateBanner } from '@/hooks/useRoomStateBanner';
import { toast } from '@/hooks/useToast';
import { logger } from '@/lib/logger';

/**
 * Unified chat surface — orchestrator. Polls platform status once via
 * `useChatPlatformStatus` and threads `statuses` + `activeStreamCount`
 * down to the composer + search modal so they don't each spin their own
 * poller. Each sub-component is a focused, self-contained module under
 * `components/chat/`.
 */
export function Chat() {
  const { t } = useTranslation();
  const { FileBrowser, saveFilePath: browserSaveFile } = useFileBrowser();
  const messages = useChatStore((state) => state.messages);
  const clearMessages = useChatStore((state) => state.clearMessages);
  const { statuses } = useChatPlatformStatus();
  const roomMode = useRoomStateBanner(messages);
  const [searchOpen, setSearchOpen] = useState(false);

  const handleExportLog = useCallback(async (): Promise<void> => {
    try {
      // Chat history is always-on (encrypted, stream-decoupled) and export
      // covers the FULL retained history across restarts — not a single
      // session — so the filename is stamped with the export moment, not a
      // session range. Emptiness is the server's call: it returns a
      // `no_chat_history` validation error, surfaced below. No client-side
      // session gate (it would duplicate the authoritative server check).
      const defaultName = `chatlog_export_${formatTimestampForFile(new Date())}.jsonl`;

      const path = await browserSaveFile({
        defaultPath: defaultName,
        filters: [{ name: 'JSONL', extensions: ['jsonl'] }],
      });

      if (!path) return;

      await api.chat.exportLog(path);
      toast.success(t('chat.exportSuccess', { defaultValue: 'Chat log exported.' }));
    } catch (error) {
      logger.error('Failed to export chat log:', error);
      // The server returns `ValidationFailed { reasons: [{ code:
      // "no_chat_history" }] }` for an empty history. The api-client
      // attaches that body's `details` to the thrown Error; the code is
      // there, not in `.message` (which is the variant tag).
      if (hasValidationCode(error, 'no_chat_history')) {
        toast.error(
          t('chat.exportNoSession', {
            defaultValue: 'No chat history to export yet.',
          })
        );
        return;
      }
      toast.error(t('chat.exportFailed', { defaultValue: 'Failed to export chat log.' }));
    }
  }, [browserSaveFile, t]);

  return (
    <>
      <FileBrowser />
      <Card className="h-full min-h-0 flex flex-col">
        <div className="border-b border-border-muted py-5 px-6 flex flex-col gap-2">
          <div className="flex items-center justify-between gap-3">
            <CardTitle>{t('chat.viewTitle', { defaultValue: 'Unified Chat' })}</CardTitle>
            <ChatOverlayHeaderButtons />
          </div>
          <CardDescription className="mt-0">
            {t('chat.viewDescription', {
              defaultValue:
                'Unified chat from your connected platforms. Sending uses your enabled accounts.',
            })}
          </CardDescription>
        </div>
        <CardBody className="p-4 flex-1 min-h-0 flex flex-col">
          <ChatOverlayToggles />

          <div className="mt-3 flex items-center gap-1">
            <Button
              variant="ghost"
              size="icon"
              onClick={clearMessages}
              aria-label={t('common.clear')}
              title={t('common.clear')}
            >
              <Trash2 className="w-4 h-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              onClick={handleExportLog}
              aria-label={t('chat.exportLog', { defaultValue: 'Export chat' })}
              title={t('chat.exportLog', { defaultValue: 'Export chat' })}
            >
              <Download className="w-4 h-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              onClick={() => setSearchOpen(true)}
              aria-label={t('chat.search', { defaultValue: 'Search' })}
              title={t('chat.search', { defaultValue: 'Search' })}
            >
              <Search className="w-4 h-4" />
            </Button>
          </div>

          <ChannelModeBanner mode={roomMode} />

          <div className="mt-3 flex-1 min-h-0 flex flex-col rounded-lg border border-border-subtle bg-bg-elevated p-2">
            <ChatList
              messages={messages}
              className="flex-1 min-h-0"
              emptyLabel={t('chat.empty', { defaultValue: 'No chat messages yet.' })}
              showTimestamps
            />
          </div>

          <ChatReauthBanner statuses={statuses} />

          <div className="mt-3 shrink-0">
            <ChatComposer statuses={statuses} />
          </div>
        </CardBody>

        <ChatSearch
          open={searchOpen}
          onClose={() => setSearchOpen(false)}
          messages={messages}
        />
      </Card>
    </>
  );
}

function formatTimestampForFile(date: Date): string {
  const pad = (value: number): string => String(value).padStart(2, '0');
  return `${date.getFullYear()}${pad(date.getMonth() + 1)}${pad(date.getDate())}-${pad(
    date.getHours()
  )}${pad(date.getMinutes())}${pad(date.getSeconds())}`;
}

/**
 * True when `error` is an api-client error carrying a `ValidationFailed`
 * body whose `reasons` include `code`. The api-client attaches the parsed
 * error body's `details` to the thrown Error.
 */
function hasValidationCode(error: unknown, code: string): boolean {
  if (!(error instanceof Error)) return false;
  const details = (error as { details?: unknown }).details;
  if (!details || typeof details !== 'object') return false;
  const reasons = (details as { reasons?: unknown }).reasons;
  if (!Array.isArray(reasons)) return false;
  return reasons.some(
    (reason): boolean =>
      typeof reason === 'object' &&
      reason !== null &&
      (reason as { code?: unknown }).code === code
  );
}
