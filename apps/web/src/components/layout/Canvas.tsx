import React, { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { ChevronLeft, ChevronRight } from 'lucide-react';
import { cn } from '@/lib/cn';

interface CanvasProps {
  input: React.ReactNode;
  pipeline: React.ReactNode;
  chat: React.ReactNode;
  chatCollapsed: boolean;
  onToggleChat: () => void;
}

const COLLAPSED_WIDTH = '40px';
const EXPANDED_WIDTH = '360px';

export function Canvas({
  input,
  pipeline,
  chat,
  chatCollapsed,
  onToggleChat,
}: CanvasProps): React.ReactElement {
  const { t } = useTranslation();
  const handleToggle = useCallback(() => onToggleChat(), [onToggleChat]);

  return (
    <main
      id="main-content"
      tabIndex={-1}
      className={cn(
        'grid h-[calc(100vh-var(--menubar-h,40px)-var(--statusstrip-h,56px))]',
        'grid-cols-[280px_minmax(0,1fr)_var(--chat-w)]',
        'overflow-hidden',
      )}
      style={
        {
          '--chat-w': chatCollapsed ? COLLAPSED_WIDTH : EXPANDED_WIDTH,
        } as React.CSSProperties
      }
    >
      <section
        aria-label={t('a11y.inputColumn', { defaultValue: 'Input' })}
        className="border-e border-border-default overflow-y-auto p-4 bg-bg-base"
      >
        {input}
      </section>

      <section
        aria-label={t('a11y.pipelineColumn', { defaultValue: 'Pipeline' })}
        className="overflow-y-auto p-4 bg-bg-base"
      >
        {pipeline}
      </section>

      <aside
        aria-label={t('a11y.chatColumn', { defaultValue: 'Chat' })}
        className="border-s border-border-default overflow-hidden flex flex-col bg-bg-base"
      >
        <ChatCollapseToggle collapsed={chatCollapsed} onToggle={handleToggle} />
        {!chatCollapsed && <div className="flex-1 overflow-y-auto">{chat}</div>}
      </aside>
    </main>
  );
}

interface ChatCollapseToggleProps {
  collapsed: boolean;
  onToggle: () => void;
}

function ChatCollapseToggle({ collapsed, onToggle }: ChatCollapseToggleProps): React.ReactElement {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      onClick={onToggle}
      aria-label={
        collapsed
          ? t('a11y.expandChat', { defaultValue: 'Expand chat panel' })
          : t('a11y.collapseChat', { defaultValue: 'Collapse chat panel' })
      }
      aria-expanded={!collapsed}
      className={cn(
        'flex items-center justify-center h-10 w-full',
        'border-b border-border-default text-text-tertiary',
        'hover:bg-bg-hover hover:text-text-primary',
        'focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default',
        'transition-colors',
      )}
    >
      {collapsed ? <ChevronLeft className="w-4 h-4" /> : <ChevronRight className="w-4 h-4" />}
    </button>
  );
}
