import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { MessageSquare, Radio, Send } from 'lucide-react';
import { ObsPanel } from '@/components/integrations/ObsPanel';
import { DiscordPanel } from '@/components/integrations/DiscordPanel';
import { ChatPanel } from '@/components/integrations/ChatPanel';
import { cn } from '@/lib/cn';

type IntegrationsTab = 'chat' | 'obs' | 'discord';

export function Integrations() {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<IntegrationsTab>('chat');

  return (
    <div className="space-y-6">
      <div className="flex gap-2 border-b border-[var(--border-default)] pb-2">
        <button
          onClick={() => setActiveTab('chat')}
          className={cn(
            'flex items-center gap-2 px-4 py-2 rounded-t-lg text-sm font-medium transition-colors',
            activeTab === 'chat'
              ? 'bg-[var(--bg-surface)] text-[var(--text-primary)] border border-b-0 border-[var(--border-default)]'
              : 'text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-muted)]'
          )}
        >
          <MessageSquare className="w-4 h-4" />
          {t('integrations.tabs.chat', { defaultValue: 'Chat' })}
        </button>
        <button
          onClick={() => setActiveTab('obs')}
          className={cn(
            'flex items-center gap-2 px-4 py-2 rounded-t-lg text-sm font-medium transition-colors',
            activeTab === 'obs'
              ? 'bg-[var(--bg-surface)] text-[var(--text-primary)] border border-b-0 border-[var(--border-default)]'
              : 'text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-muted)]'
          )}
        >
          <Radio className="w-4 h-4" />
          {t('integrations.tabs.broadcast', { defaultValue: 'Broadcast' })}
        </button>
        <button
          onClick={() => setActiveTab('discord')}
          className={cn(
            'flex items-center gap-2 px-4 py-2 rounded-t-lg text-sm font-medium transition-colors',
            activeTab === 'discord'
              ? 'bg-[var(--bg-surface)] text-[var(--text-primary)] border border-b-0 border-[var(--border-default)]'
              : 'text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--bg-muted)]'
          )}
        >
          <Send className="w-4 h-4" />
          {t('integrations.tabs.social', { defaultValue: 'Social' })}
        </button>
      </div>

      {activeTab === 'chat' && <ChatPanel />}
      {activeTab === 'obs' && <ObsPanel />}
      {activeTab === 'discord' && <DiscordPanel />}
    </div>
  );
}
