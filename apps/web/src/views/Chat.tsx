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
import { ChatSearch } from '@/components/chat/ChatSearch';
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
  const { statuses, activeStreamCount } = useChatPlatformStatus();
  const [searchOpen, setSearchOpen] = useState(false);

  const handleExportLog = useCallback(async (): Promise<void> => {
    try {
      if (activeStreamCount === 0) {
        toast.error(
          t('chat.exportRequiresStream', {
            defaultValue: 'Start a stream to export the current chat session.',
          })
        );
        return;
      }

      const status = await api.chat.getLogStatus();
      if (!status.active || status.startedAt === 0) {
        toast.error(
          t('chat.exportNoSession', {
            defaultValue: 'No active chat session to export.',
          })
        );
        return;
      }

      // `startedAt` is `bigint` (ts-rs maps Rust `i64` to bigint). The
      // `Date` constructor needs a `number` — Unix epoch ms safely fits
      // in JS `number` for any timestamp before year ~285,000.
      const start = new Date(Number(status.startedAt));
      const end = new Date();
      const defaultName = `chatlog_${formatTimestampForFile(start)}_to_${formatTimestampForFile(
        end
      )}.jsonl`;

      const path = await browserSaveFile({
        defaultPath: defaultName,
        filters: [{ name: 'JSONL', extensions: ['jsonl'] }],
      });

      if (!path) return;

      await api.chat.exportLog(path);
      toast.success(t('chat.exportSuccess', { defaultValue: 'Chat log exported.' }));
    } catch (error) {
      logger.error('Failed to export chat log:', error);
      toast.error(t('chat.exportFailed', { defaultValue: 'Failed to export chat log.' }));
    }
  }, [activeStreamCount, browserSaveFile, t]);

  return (
    <>
      <FileBrowser />
      <Card>
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
        <CardBody className="p-4">
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
              disabled={activeStreamCount === 0}
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

          <div className="mt-3 rounded-lg border border-border-subtle bg-bg-elevated p-2">
            <ChatList
              messages={messages}
              className="max-h-[520px]"
              emptyLabel={t('chat.empty', { defaultValue: 'No chat messages yet.' })}
              showTimestamps
            />
          </div>

          <div className="mt-3">
            <ChatComposer statuses={statuses} />
          </div>
        </CardBody>

        <ChatSearch
          open={searchOpen}
          onClose={() => setSearchOpen(false)}
          messages={messages}
          activeStreamCount={activeStreamCount}
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
