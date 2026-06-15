import React, { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Pencil, Trash2, MessageSquare } from 'lucide-react';
import { cn } from '@/lib/cn';
import { api } from '@/lib/client';
import { toast } from '@/hooks/useToast';
import { logger } from '@/lib/logger';
import type { ChatPlatform, ChatPlatformStatus, StreamTarget } from '@spiritstream/types';
import { serviceToChatPlatform } from '@/lib/serviceChat';
import { PlatformIcon } from '@/components/stream/PlatformIcon';

export type OutputRowStatus = 'live' | 'connecting' | 'offline' | 'error';

interface OutputRowProps {
  target: StreamTarget;
  status: OutputRowStatus;
  enabled: boolean;
  onToggleEnabled: () => void;
  onEdit: () => void;
  onRemove: () => void;
  /** Jump to this service's chat settings. Shown only when the target's
   *  service has a chat integration available for setup. */
  onOpenChatSettings?: (platform: ChatPlatform) => void;
  /** Resolve the live chat status for a platform, or null when the platform
   *  isn't set up yet. Non-null → the connect/disconnect toggle is shown. */
  chatConnectionFor?: (platform: ChatPlatform) => ChatPlatformStatus['status'] | null;
}

const STATUS_DOT_CLASS: Record<OutputRowStatus, string> = {
  live: 'bg-error-text animate-pulse',
  connecting: 'bg-warning-text animate-pulse',
  offline: 'bg-text-muted',
  error: 'bg-error-border',
};

export function OutputRow({
  target,
  status,
  enabled,
  onToggleEnabled,
  onEdit,
  onRemove,
  onOpenChatSettings,
  chatConnectionFor,
}: OutputRowProps): React.ReactElement {
  const { t } = useTranslation();
  const chatPlatform = serviceToChatPlatform(target.service);
  const chatStatus = chatPlatform ? (chatConnectionFor?.(chatPlatform) ?? null) : null;

  const chatIconButton =
    chatPlatform && onOpenChatSettings ? (
      <button
        type="button"
        onClick={() => onOpenChatSettings(chatPlatform)}
        aria-label={t('pipeline.target.chatSettings', {
          defaultValue: 'Chat settings for {{name}}',
          name: target.name,
        })}
        title={t('pipeline.target.chatSettings', {
          defaultValue: 'Chat settings for {{name}}',
          name: target.name,
        })}
        className={cn(
          'inline-flex items-center justify-center w-8 h-8 rounded-full',
          'text-text-tertiary hover:bg-bg-hover hover:text-text-primary',
          'focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default',
          'transition-colors'
        )}
      >
        <MessageSquare className="w-4 h-4" />
      </button>
    ) : null;

  return (
    <li
      className={cn(
        'flex items-center gap-3 px-3 py-2 rounded-md',
        'bg-bg-surface border border-border-muted',
        'hover:border-border-default transition-colors'
      )}
    >
      <span
        className={cn('w-2.5 h-2.5 rounded-full flex-shrink-0', STATUS_DOT_CLASS[status])}
        aria-hidden="true"
      />

      {/* Brand-colored service badge. Decorative (aria-hidden inside); the
          `target.service` subtitle below carries the accessible platform label. */}
      <PlatformIcon platform={target.service} size="sm" className="flex-shrink-0" />

      <div className="flex-1 min-w-0 flex flex-col">
        <span className="text-sm font-medium text-text-primary truncate">{target.name}</span>
        <span className="text-xs text-text-tertiary truncate">{target.service}</span>
      </div>

      <span className="text-xs text-text-tertiary tabular-nums hidden sm:inline">
        {t(`status.${status}`, { defaultValue: status })}
      </span>

      <EnabledSwitch enabled={enabled} onToggle={onToggleEnabled} targetName={target.name} />

      {/* Chat controls grouped by a "wire" (bordered pill) once set up, so the
          settings icon + connect/disconnect toggle read as one chat unit. */}
      {chatPlatform && chatStatus !== null ? (
        <div className="inline-flex items-center gap-1 rounded-full border border-border-default ps-0.5 pe-1.5 py-0.5">
          {chatIconButton}
          <RowChatToggle platform={chatPlatform} status={chatStatus} targetName={target.name} />
        </div>
      ) : (
        chatIconButton
      )}

      <button
        type="button"
        onClick={onEdit}
        aria-label={t('pipeline.target.edit', {
          defaultValue: 'Edit {{name}}',
          name: target.name,
        })}
        className={cn(
          'inline-flex items-center justify-center w-8 h-8 rounded-md',
          'text-text-tertiary hover:bg-bg-hover hover:text-text-primary',
          'focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default',
          'transition-colors'
        )}
      >
        <Pencil className="w-4 h-4" />
      </button>

      <button
        type="button"
        onClick={onRemove}
        aria-label={t('pipeline.target.remove', {
          defaultValue: 'Remove {{name}}',
          name: target.name,
        })}
        className={cn(
          'inline-flex items-center justify-center w-8 h-8 rounded-md',
          'text-text-tertiary hover:bg-error-subtle hover:text-error-text',
          'focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default',
          'transition-colors'
        )}
      >
        <Trash2 className="w-4 h-4" />
      </button>
    </li>
  );
}

interface RowSwitchProps {
  checked: boolean;
  onToggle: () => void;
  ariaLabel: string;
  title?: string;
  disabled?: boolean;
  /** On-state track color class. */
  onColorClass: string;
}

/**
 * The single row toggle primitive — shared by the target-enabled switch and
 * the chat connect/disconnect switch so they are pixel-identical. Geometry
 * matches the global `ui/Toggle` (w-11 h-6, 18px knob, translate-x-5); only the
 * on-color varies (violet for enabled, green for chat-connected).
 */
function RowSwitch({
  checked,
  onToggle,
  ariaLabel,
  title,
  disabled,
  onColorClass,
}: RowSwitchProps): React.ReactElement {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={ariaLabel}
      title={title}
      onClick={onToggle}
      disabled={disabled}
      className={cn(
        'relative inline-flex w-11 h-6 rounded-full transition-colors flex-shrink-0',
        'focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default focus-visible:ring-offset-2',
        checked ? onColorClass : 'bg-border-strong',
        disabled && 'opacity-60'
      )}
    >
      <span
        aria-hidden="true"
        className={cn(
          'absolute top-[3px] start-[3px] w-[18px] h-[18px] rounded-full bg-white shadow-sm',
          'transition-transform duration-200',
          checked ? 'translate-x-5' : 'translate-x-0'
        )}
      />
    </button>
  );
}

interface RowChatToggleProps {
  platform: ChatPlatform;
  status: ChatPlatformStatus['status'];
  targetName: string;
}

/**
 * Per-row chat connect/disconnect toggle — the "go silent" control surfaced
 * next to the chat-settings icon once a service's chat is set up. Connected
 * shows green (distinct from the violet target-enabled switch). The backend
 * builds credentials from the active profile and records a deliberate
 * disconnect; the visual flips on the next status poll (owned by PipelineColumn).
 */
function RowChatToggle({ platform, status, targetName }: RowChatToggleProps): React.ReactElement {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  const connected = status === 'connected' || status === 'connecting';

  const handleToggle = useCallback(async (): Promise<void> => {
    setBusy(true);
    try {
      if (connected) {
        await api.chat.disconnect(platform);
      } else {
        await api.chat.connectPlatform(platform);
      }
    } catch (error) {
      logger.error(`[OutputRow] ${platform} chat toggle failed:`, error);
      toast.error(
        t('chat.connect.failed', {
          defaultValue: 'Could not change connection: {{error}}',
          error: error instanceof Error ? error.message : String(error),
        })
      );
    } finally {
      setBusy(false);
    }
  }, [connected, platform, t]);

  return (
    <RowSwitch
      checked={connected}
      onToggle={handleToggle}
      disabled={busy}
      onColorClass="bg-success"
      ariaLabel={t('pipeline.target.chatToggle', {
        defaultValue: 'Chat connection for {{name}}',
        name: targetName,
      })}
      title={
        connected
          ? t('chat.connect.disconnect', { defaultValue: 'Disconnect chat' })
          : t('chat.connect.connect', { defaultValue: 'Connect chat' })
      }
    />
  );
}

interface EnabledSwitchProps {
  enabled: boolean;
  onToggle: () => void;
  targetName: string;
}

function EnabledSwitch({ enabled, onToggle, targetName }: EnabledSwitchProps): React.ReactElement {
  const { t } = useTranslation();
  return (
    <RowSwitch
      checked={enabled}
      onToggle={onToggle}
      onColorClass="bg-primary"
      ariaLabel={t('pipeline.target.enableLabel', {
        defaultValue: 'Enable {{name}}',
        name: targetName,
      })}
    />
  );
}
