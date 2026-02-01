import { ObsPanel } from '@/components/integrations/ObsPanel';
import { DiscordPanel } from '@/components/integrations/DiscordPanel';
import { ChatPanel } from '@/components/integrations/ChatPanel';

export function Integrations() {
  return (
    <div className="space-y-8">
      <ChatPanel />
      <hr className="border-[var(--border-muted)]" />
      <ObsPanel />
      <hr className="border-[var(--border-muted)]" />
      <DiscordPanel />
    </div>
  );
}
