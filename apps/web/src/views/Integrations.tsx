import { ObsPanel } from '@/components/integrations/ObsPanel';
import { DiscordPanel } from '@/components/integrations/DiscordPanel';

export function Integrations() {
  return (
    <div className="space-y-8">
      <ObsPanel />
      <hr className="border-[var(--border-muted)]" />
      <DiscordPanel />
    </div>
  );
}
