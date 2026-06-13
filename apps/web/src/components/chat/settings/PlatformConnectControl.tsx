import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Plug, PlugZap } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { api } from '@/lib/client';
import { toast } from '@/hooks/useToast';
import { logger } from '@/lib/logger';
import type { ChatPlatform, ChatPlatformStatus } from '@spiritstream/types';

interface PlatformConnectControlProps {
  platform: ChatPlatform;
  /** Live status from the parent's `useChatPlatformStatus`. */
  status: ChatPlatformStatus['status'];
}

/**
 * Per-platform Connect / Disconnect toggle — the "go silent" control.
 * Chat is decoupled from streaming, so the user can connect a platform's
 * chat on demand or disconnect it to stop receiving stranger messages,
 * independent of whether they're live. The backend builds credentials
 * from the active profile (thin frontend) and records a deliberate
 * disconnect so auto-connect/reconnect won't undo it. The badge updates
 * on the next status poll.
 */
export function PlatformConnectControl({
  platform,
  status,
}: PlatformConnectControlProps): React.ReactElement {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  const connected = status === 'connected' || status === 'connecting';

  const handleToggle = async (): Promise<void> => {
    setBusy(true);
    try {
      if (connected) {
        await api.chat.disconnect(platform);
        toast.success(t('chat.connect.disconnected', { defaultValue: 'Disconnected' }));
      } else {
        await api.chat.connectPlatform(platform);
        toast.success(t('chat.connect.connecting', { defaultValue: 'Connecting…' }));
      }
    } catch (error) {
      logger.error(`[PlatformConnectControl] ${platform} toggle failed:`, error);
      toast.error(
        t('chat.connect.failed', {
          defaultValue: 'Could not change connection: {{error}}',
          error: error instanceof Error ? error.message : String(error),
        })
      );
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="mt-3">
      <Button variant={connected ? 'ghost' : 'secondary'} size="sm" onClick={handleToggle} disabled={busy}>
        {connected ? <PlugZap className="w-3.5 h-3.5" /> : <Plug className="w-3.5 h-3.5" />}
        {connected
          ? t('chat.connect.disconnect', { defaultValue: 'Disconnect chat' })
          : t('chat.connect.connect', { defaultValue: 'Connect chat' })}
      </Button>
    </div>
  );
}
