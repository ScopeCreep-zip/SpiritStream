import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/cn';
import { StreamStatus } from '@/components/ui/StreamStatus';
import { PlatformIcon } from '@/components/stream/PlatformIcon';
import type { Platform } from '@/types/profile';

export interface ProfileCardMeta {
  icon: React.ReactNode;
  label: string;
}

export interface ProfileCardProps {
  name: string;
  meta: ProfileCardMeta[];
  active?: boolean;
  onClick?: () => void;
  actions?: React.ReactNode;
  /** Services/platforms configured in this profile (Story 1.1, 4.1, 4.2) */
  services?: Platform[];
  className?: string;
}

export function ProfileCard({
  name,
  meta,
  active,
  onClick,
  actions,
  services,
  className,
}: ProfileCardProps) {
  const { t } = useTranslation();

  return (
    <div
      onClick={onClick}
      className={cn(
        'bg-[var(--bg-surface)] border-2 rounded-xl h-full',
        'transition-all duration-150',
        onClick && 'cursor-pointer',
        active
          ? 'border-[var(--primary)] bg-[var(--primary-muted)]'
          : 'border-[var(--border-default)] hover:border-[var(--border-interactive)] hover:-translate-y-0.5 hover:shadow-[var(--shadow-md)]',
        className
      )}
      style={{ padding: '20px' }}
    >
      <div className="flex items-center justify-between mb-3">
        <span className="font-semibold text-[var(--text-primary)]">{name}</span>
        <div className="flex items-center gap-2">
          {active && <StreamStatus status="live" label={t('dashboard.active')} />}
          {actions}
        </div>
      </div>
      <div className="flex gap-4 text-small text-[var(--text-secondary)]">
        {meta.map((item, index) => (
          <span key={index} className="flex items-center gap-1.5">
            {item.icon}
            {item.label}
          </span>
        ))}
      </div>
      {/* Service icons showing which platforms this profile targets (Story 1.1, 4.1, 4.2) */}
      {services && services.length > 0 && (
        <div className="flex gap-1.5 mt-3 pt-3 border-t border-[var(--border-muted)]">
          {services.map((service) => (
            <PlatformIcon key={service} platform={service} size="sm" />
          ))}
        </div>
      )}
    </div>
  );
}
