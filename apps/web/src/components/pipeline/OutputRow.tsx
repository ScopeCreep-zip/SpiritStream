import React from 'react';
import { useTranslation } from 'react-i18next';
import { Pencil, Trash2 } from 'lucide-react';
import { cn } from '@/lib/cn';
import type { StreamTarget } from '@spiritstream/types';

export type OutputRowStatus = 'live' | 'connecting' | 'offline' | 'error';

interface OutputRowProps {
  target: StreamTarget;
  status: OutputRowStatus;
  enabled: boolean;
  onToggleEnabled: () => void;
  onEdit: () => void;
  onRemove: () => void;
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
}: OutputRowProps): React.ReactElement {
  const { t } = useTranslation();

  return (
    <li
      className={cn(
        'flex items-center gap-3 px-3 py-2 rounded-md',
        'bg-bg-surface border border-border-muted',
        'hover:border-border-default transition-colors',
      )}
    >
      <span
        className={cn('w-2.5 h-2.5 rounded-full flex-shrink-0', STATUS_DOT_CLASS[status])}
        aria-hidden="true"
      />

      <div className="flex-1 min-w-0 flex flex-col">
        <span className="text-sm font-medium text-text-primary truncate">{target.name}</span>
        <span className="text-xs text-text-tertiary truncate">{target.service}</span>
      </div>

      <span className="text-xs text-text-tertiary tabular-nums hidden sm:inline">
        {t(`status.${status}`, { defaultValue: status })}
      </span>

      <EnabledSwitch
        enabled={enabled}
        onToggle={onToggleEnabled}
        targetName={target.name}
      />

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
          'transition-colors',
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
          'transition-colors',
        )}
      >
        <Trash2 className="w-4 h-4" />
      </button>
    </li>
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
    <button
      type="button"
      role="switch"
      aria-checked={enabled}
      aria-label={t('pipeline.target.enableLabel', {
        defaultValue: 'Enable {{name}}',
        name: targetName,
      })}
      onClick={onToggle}
      className={cn(
        'relative inline-flex w-10 h-6 rounded-full transition-colors flex-shrink-0',
        'focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default focus-visible:ring-offset-2',
        enabled ? 'bg-primary' : 'bg-border-strong',
      )}
    >
      <span
        aria-hidden="true"
        className={cn(
          'absolute top-[3px] start-[3px] w-[18px] h-[18px] rounded-full bg-white shadow-sm',
          'transition-transform duration-200',
          enabled ? 'translate-x-4' : 'translate-x-0',
        )}
      />
    </button>
  );
}
