import React, { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Play, Square, Shield, ChevronDown, Loader2 } from 'lucide-react';
import { api } from '@/lib/client';
import { toast } from '@/hooks/useToast';
import { logger } from '@/lib/logger';
import { useStreamStore } from '@/stores/streamStore';
import { incomingRtmpUrl } from '@/lib/profile-helpers';
import { formatUptime, formatBitrate } from '@/hooks/useStreamStats';
import { ConnectionStatus } from '@/components/ui/ConnectionStatus';
import { cn } from '@/lib/cn';
import type { Profile } from '@spiritstream/types';
import type { ModalName } from '@/hooks/useModalRegistry';

interface StatusStripProps {
  profile: Profile | null;
  onOpenModal: (name: ModalName) => void;
}

export function StatusStrip({ profile, onOpenModal }: StatusStripProps): React.ReactElement {
  const { t } = useTranslation();
  const isStreaming = useStreamStore((s) => s.isStreaming);
  const globalStatus = useStreamStore((s) => s.globalStatus);
  const uptime = useStreamStore((s) => s.uptime);
  const activeStreamCount = useStreamStore((s) => s.activeStreamCount);
  const stats = useStreamStore((s) => s.stats);
  const startAllGroups = useStreamStore((s) => s.startAllGroups);
  const stopAllGroups = useStreamStore((s) => s.stopAllGroups);

  const [starting, setStarting] = useState(false);
  const [panicking, setPanicking] = useState(false);

  const handleStart = useCallback(async (): Promise<void> => {
    if (!profile) return;
    setStarting(true);
    try {
      await api.stream.validate(profile);
      await startAllGroups(profile.outputGroups, incomingRtmpUrl(profile.input));
      toast.success(t('toast.streamStarted'));
    } catch (err) {
      logger.error('[status-strip] start failed', err);
      toast.error(
        t('toast.startFailed', { error: err instanceof Error ? err.message : String(err) })
      );
    } finally {
      setStarting(false);
    }
  }, [profile, startAllGroups, t]);

  const handlePanic = useCallback(async (): Promise<void> => {
    setPanicking(true);
    try {
      const result = await api.safety.panic();
      toast.success(
        t('toast.panicStopped', {
          count: result.streamsStopped,
          ms: result.elapsedMs,
          defaultValue: 'Panic disconnect: stopped {{count}} streams in {{ms}}ms',
        })
      );
    } catch (err) {
      logger.error('[status-strip] panic failed', err);
      toast.error(
        t('toast.panicFailed', {
          defaultValue: 'Panic disconnect failed: {{error}}',
          error: err instanceof Error ? err.message : String(err),
        })
      );
    } finally {
      setPanicking(false);
    }
  }, [t]);

  return (
    <div
      role="region"
      aria-label={t('a11y.statusStrip', { defaultValue: 'Stream status and controls' })}
      aria-live="polite"
      className="flex items-center gap-4 px-4 h-[var(--statusstrip-h,56px)] bg-bg-surface border-b border-border-default"
    >
      <ConnectionStatus />

      <ProfilePill profile={profile} onClick={() => onOpenModal('openProfile')} />

      <LiveBadge status={globalStatus} uptime={uptime} activeCount={activeStreamCount} />

      {isStreaming && (
        <span className="text-sm text-text-secondary tabular-nums">
          {stats.totalBitrate > 0 ? formatBitrate(stats.totalBitrate) : '0 kbps'}
        </span>
      )}

      <div className="flex-1" />

      {isStreaming ? (
        <button
          type="button"
          onClick={() => stopAllGroups()}
          className={cn(
            'inline-flex items-center gap-2 px-4 h-10 min-w-[44px]',
            'rounded-md bg-error-bg text-error-text border border-error-border',
            'hover:bg-error-bg/80 focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default',
            'font-medium text-sm transition-colors'
          )}
        >
          <Square className="w-4 h-4" aria-hidden="true" />
          {t('streams.stopStreaming', { defaultValue: 'Stop' })}
        </button>
      ) : (
        <button
          type="button"
          onClick={handleStart}
          disabled={!profile || starting}
          className={cn(
            'inline-flex items-center gap-2 px-4 h-10 min-w-[44px]',
            'rounded-md bg-primary text-primary-foreground',
            'hover:bg-primary-hover focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default',
            'disabled:opacity-50 disabled:cursor-not-allowed',
            'font-medium text-sm transition-colors'
          )}
        >
          {starting ? (
            <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
          ) : (
            <Play className="w-4 h-4" aria-hidden="true" />
          )}
          {starting
            ? t('streams.validating', { defaultValue: 'Validating…' })
            : t('streams.startStreaming', { defaultValue: 'Start' })}
        </button>
      )}

      <button
        type="button"
        onClick={handlePanic}
        disabled={panicking}
        aria-label={t('a11y.panic', {
          defaultValue: 'Panic disconnect — stop all streams immediately (⌘P)',
        })}
        className={cn(
          'inline-flex items-center justify-center h-12 w-12',
          'rounded-md bg-error-bg text-error-text border-2 border-error-border',
          'hover:bg-error-border hover:text-text-inverse',
          'focus-visible:outline-none focus-visible:ring-[4px] focus-visible:ring-error-border focus-visible:ring-offset-2',
          'disabled:opacity-70 disabled:cursor-wait',
          'font-bold transition-colors'
        )}
      >
        {panicking ? <Loader2 className="w-5 h-5 animate-spin" /> : <Shield className="w-5 h-5" />}
      </button>
    </div>
  );
}

interface ProfilePillProps {
  profile: Profile | null;
  onClick: () => void;
}

function ProfilePill({ profile, onClick }: ProfilePillProps): React.ReactElement {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        'inline-flex items-center gap-2 px-3 h-9 rounded-md',
        'bg-bg-muted text-text-primary border border-border-default',
        'hover:bg-bg-hover focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default',
        'text-sm font-medium transition-colors'
      )}
    >
      <span className="w-2 h-2 rounded-full bg-primary" aria-hidden="true" />
      <span>{profile?.name ?? t('status.noProfile', { defaultValue: 'No profile' })}</span>
      <ChevronDown className="w-4 h-4 text-text-tertiary" aria-hidden="true" />
    </button>
  );
}

interface LiveBadgeProps {
  status: string;
  uptime: number;
  activeCount: number;
}

function LiveBadge({ status, uptime, activeCount }: LiveBadgeProps): React.ReactElement {
  const { t } = useTranslation();
  const isLive = status === 'live';
  return (
    <div className="inline-flex items-center gap-2 text-sm">
      <span
        className={cn(
          'w-2.5 h-2.5 rounded-full',
          isLive ? 'bg-error-text animate-pulse' : 'bg-text-muted'
        )}
        aria-hidden="true"
      />
      <span className={cn('font-medium', isLive ? 'text-error-text' : 'text-text-secondary')}>
        {isLive
          ? t('status.live', { defaultValue: 'LIVE' })
          : t('status.offline', { defaultValue: 'Offline' })}
      </span>
      {isLive && (
        <>
          <span className="text-text-tertiary tabular-nums">
            {formatUptime(Math.floor(uptime))}
          </span>
          <span className="text-text-tertiary">
            ·{' '}
            {t('status.activeTargets', {
              count: activeCount,
              defaultValue: '{{count}} active',
            })}
          </span>
        </>
      )}
    </div>
  );
}
