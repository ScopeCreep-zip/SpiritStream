import { useEffect, useState, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Play, Square, Settings2, Activity, Gauge, Clock, Upload, Radio, AlertTriangle } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Alert } from '@/components/ui/Alert';
import { Toggle } from '@/components/ui/Toggle';
import { StatsRow } from '@/components/dashboard/StatsRow';
import { StatBox } from '@/components/dashboard/StatBox';
import { OutputGroup } from '@/components/stream/OutputGroup';
import { PlatformIcon } from '@/components/stream/PlatformIcon';
import { useProfileStore } from '@/stores/profileStore';
import { useStreamStore } from '@/stores/streamStore';
import { formatUptime, formatBitrate } from '@/hooks/useStreamStats';
import { toast } from '@/hooks/useToast';
import type { View } from '@/App';
import type { OutputGroup as OutputGroupType, StreamTarget } from '@/types/profile';
import { validateStreamConfig, displayValidationIssues } from '@/lib/streamValidation';

interface StreamManagerProps {
  onNavigate: (view: View) => void;
}

export function StreamManager({ onNavigate }: StreamManagerProps) {
  const { t } = useTranslation();
  const { current, loading, error } = useProfileStore();
  const {
    isStreaming,
    activeGroups,
    enabledTargets,
    globalStatus,
    groupStats,
    error: streamError,
    stats,
    uptime,
    activeStreamCount,
    startAllGroups,
    stopAllGroups,
    setTargetEnabled,
  } = useStreamStore();

  const [isValidating, setIsValidating] = useState(false);

  // Enable all targets by default when profile changes
  useEffect(() => {
    if (current) {
      const allTargetIds = current.outputGroups.flatMap((g) => g.streamTargets.map((t) => t.id));
      // Only add targets that aren't already in enabledTargets
      allTargetIds.forEach((id) => {
        if (!enabledTargets.has(id)) {
          setTargetEnabled(id, true);
        }
      });
    }
    // Only re-run when profile ID changes, not on every target enable/disable
    // current/enabledTargets/setTargetEnabled are intentionally excluded
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [current?.id]);

  const handleStartAll = async () => {
    if (!current) {
      return;
    }

    setIsValidating(true);

    try {
      // Run comprehensive validation (including FFmpeg check)
      const result = await validateStreamConfig(current, {
        checkFfmpeg: true,
        checkEnabledTargetsOnly: true,
        enabledTargetIds: enabledTargets,
      });

      if (!result.valid) {
        displayValidationIssues(result.issues, toast);
        return;
      }

      // Validation passed, start streaming
      // Build incoming URL from structured input
      const incomingUrl = `rtmp://${current.input.bindAddress}:${current.input.port}/${current.input.application}`;
      await startAllGroups(current.outputGroups, incomingUrl);
      toast.success(t('toast.streamStarted'));
    } catch (err) {
      console.error('[StreamManager] startAllGroups failed:', err);
      toast.error(
        t('toast.startFailed', { error: err instanceof Error ? err.message : String(err) })
      );
    } finally {
      setIsValidating(false);
    }
  };

  const handleStopAll = async () => {
    try {
      await stopAllGroups();
      toast.info(t('toast.streamStopped'));
    } catch (err) {
      toast.error(
        t('toast.stopFailed', { error: err instanceof Error ? err.message : String(err) })
      );
    }
  };

  const isConnecting = globalStatus === 'connecting';
  const activeTargetCount = useMemo(() => {
    if (!current) return 0;

    return current.outputGroups.reduce((total, group) => {
      if (!activeGroups.has(group.id)) {
        return total;
      }
      const enabledCount = group.streamTargets.filter((t) => enabledTargets.has(t.id)).length;
      return total + enabledCount;
    }, 0);
  }, [current, activeGroups, enabledTargets]);

  const displayActiveCount =
    activeTargetCount > 0 ? activeTargetCount : activeStreamCount > 0 ? activeStreamCount : activeGroups.size;

  // Calculate total bandwidth for enabled groups
  const totalBandwidth = useMemo(() => {
    if (!current) return 0;

    return current.outputGroups.reduce((total: number, group: OutputGroupType) => {
      // Check if any targets in this group are enabled
      const hasEnabledTargets = group.streamTargets.some((t: StreamTarget) =>
        enabledTargets.has(t.id)
      );
      if (!hasEnabledTargets) return total;

      // Count enabled targets in this group (each target needs the full bitrate)
      const enabledCount = group.streamTargets.filter((t: StreamTarget) =>
        enabledTargets.has(t.id)
      ).length;

      // Add video + audio bitrate for each enabled target
      // Parse bitrate strings (e.g., "6000k" -> 6000)
      const videoBitrate = parseInt(group.video.bitrate.replace(/[^\d]/g, ''), 10) || 0;
      const audioBitrate = parseInt(group.audio.bitrate.replace(/[^\d]/g, ''), 10) || 0;
      const groupBitrate = (videoBitrate + audioBitrate) * enabledCount;
      return total + groupBitrate;
    }, 0);
  }, [current, enabledTargets]);

  // Format bandwidth for display
  const formatTotalBandwidth = (kbps: number): string => {
    if (kbps >= 1000) {
      return `${(kbps / 1000).toFixed(1)} Mbps`;
    }
    return `${kbps} kbps`;
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-[var(--text-secondary)]">{t('common.loading')}</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-[var(--error-text)]">
          {t('common.error')}: {error}
        </div>
      </div>
    );
  }

  if (!current) {
    return (
      <Card>
        <CardBody>
          <div className="text-center py-12">
            <p className="text-[var(--text-secondary)]">{t('streams.selectProfileFirst')}</p>
          </div>
        </CardBody>
      </Card>
    );
  }

  const outputGroups = current.outputGroups;

  if (outputGroups.length === 0) {
    return (
      <Alert variant="warning" title={t('streams.noOutputGroups')}>
        {t('streams.noOutputGroupsDescription')}
      </Alert>
    );
  }

  // Get status for each group
  const getGroupStatus = (groupId: string): 'live' | 'offline' | 'error' => {
    if (activeGroups.has(groupId)) return 'live';
    return 'offline';
  };

  // Get group info string
  const getGroupInfo = (group: (typeof outputGroups)[0]): string => {
    const targetCount = group.streamTargets.length;
    const resolution = `${group.video.width}x${group.video.height}`;
    const bitrate = group.video.bitrate.replace('k', '');
    return `${targetCount} target${targetCount !== 1 ? 's' : ''} • ${resolution} • ${bitrate} kbps`;
  };

  return (
    <div className="flex flex-col" style={{ gap: '24px' }}>
      {streamError && (
        <Alert variant="error" title={t('streams.streamError')}>
          {streamError}
        </Alert>
      )}

      {!isStreaming && !streamError && (
        <Alert variant="info" title={t('streams.readyToStream')}>
          {t('streams.readyToStreamDescription')}
        </Alert>
      )}

      <StatsRow>
        <StatBox
          icon={<Radio className="w-5 h-5" />}
          label={t('dashboard.activeStreams')}
          value={displayActiveCount}
          change={isStreaming ? t('status.streaming') : t('status.readyToStart')}
          changeType={isStreaming ? 'positive' : 'neutral'}
        />
        <StatBox
          icon={<Activity className="w-5 h-5" />}
          label={t('dashboard.totalBitrate')}
          value={stats.totalBitrate > 0 ? formatBitrate(stats.totalBitrate) : '0 kbps'}
          change={isStreaming ? t('status.active') : t('status.noActiveStreams')}
        />
        <StatBox
          icon={<AlertTriangle className="w-5 h-5" />}
          label={t('dashboard.droppedFrames')}
          value={stats.droppedFrames}
          change={stats.droppedFrames === 0 ? t('status.noIssues') : t('status.checkConnection')}
          changeType={stats.droppedFrames === 0 ? 'positive' : 'neutral'}
        />
        <StatBox
          icon={<Clock className="w-5 h-5" />}
          label={t('dashboard.uptime')}
          value={formatUptime(Math.floor(uptime))}
          change={isStreaming ? t('status.live') : t('status.notStreaming')}
        />
      </StatsRow>

      <Card>
        <CardHeader>
          <div>
            <CardTitle>{t('streams.streamControl')}</CardTitle>
            <CardDescription>{t('streams.streamControlDescription')}</CardDescription>
          </div>
          {totalBandwidth > 0 && (
            <div className="flex items-center gap-2 px-3 py-1.5 bg-[var(--bg-muted)] rounded-lg border border-[var(--border-default)]">
              <Upload className="w-4 h-4 text-[var(--text-tertiary)]" />
              <div className="text-sm">
                <span className="text-[var(--text-tertiary)]">{t('streams.totalBandwidth')}:</span>
                <span className="font-semibold text-[var(--text-primary)] ml-1">
                  {formatTotalBandwidth(totalBandwidth)}
                </span>
              </div>
            </div>
          )}
        </CardHeader>
        <CardBody style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
          {outputGroups.map((group: OutputGroupType) => {
            const stats = groupStats[group.id];
            const isGroupActive = activeGroups.has(group.id);

            return (
              <OutputGroup
                key={group.id}
                name={group.name || t('streams.defaultGroupName')}
                info={getGroupInfo(group)}
                status={getGroupStatus(group.id)}
                defaultExpanded={isGroupActive}
              >
                {/* Real-time stats when streaming */}
                {isGroupActive && stats && (
                  <div className="grid grid-cols-4 gap-4 p-4 mb-4 bg-[var(--bg-muted)] rounded-lg border border-[var(--border-default)]">
                    <div className="text-center">
                      <div className="flex items-center justify-center gap-1 text-[var(--text-tertiary)] mb-1">
                        <Activity className="w-3 h-3" />
                        <span className="text-xs">{t('streams.fps')}</span>
                      </div>
                      <div className="text-lg font-semibold text-[var(--text-primary)]">
                        {stats.fps.toFixed(1)}
                      </div>
                    </div>
                    <div className="text-center">
                      <div className="flex items-center justify-center gap-1 text-[var(--text-tertiary)] mb-1">
                        <Gauge className="w-3 h-3" />
                        <span className="text-xs">{t('streams.bitrate')}</span>
                      </div>
                      <div className="text-lg font-semibold text-[var(--text-primary)]">
                        {formatBitrate(stats.bitrate)}
                      </div>
                    </div>
                    <div className="text-center">
                      <div className="flex items-center justify-center gap-1 text-[var(--text-tertiary)] mb-1">
                        <Clock className="w-3 h-3" />
                        <span className="text-xs">{t('streams.uptime')}</span>
                      </div>
                      <div className="text-lg font-semibold text-[var(--text-primary)]">
                        {formatUptime(stats.uptime)}
                      </div>
                    </div>
                    <div className="text-center">
                      <div className="text-xs text-[var(--text-tertiary)] mb-1">
                        {t('streams.speed')}
                      </div>
                      <div className="text-lg font-semibold text-[var(--text-primary)]">
                        {stats.speed.toFixed(2)}x
                      </div>
                    </div>
                  </div>
                )}

                <div
                  className="flex flex-col border-t border-[var(--border-muted)]"
                  style={{ gap: '12px', paddingTop: '12px' }}
                >
                  {group.streamTargets.map((target: StreamTarget) => (
                    <div
                      key={target.id}
                      className="flex items-center justify-between rounded-lg bg-[var(--bg-surface)] border border-[var(--border-default)]"
                      style={{ padding: '12px' }}
                    >
                      <div className="flex items-center" style={{ gap: '12px' }}>
                        <PlatformIcon platform={target.service} size="sm" />
                        <div>
                          <div className="font-medium text-sm text-[var(--text-primary)]">
                            {target.name}
                          </div>
                          <div className="text-xs text-[var(--text-tertiary)]">{target.url}</div>
                        </div>
                      </div>
                      <Toggle
                        checked={enabledTargets.has(target.id)}
                        onChange={(checked: boolean) => setTargetEnabled(target.id, checked)}
                        disabled={isStreaming}
                      />
                    </div>
                  ))}

                  {group.streamTargets.length === 0 && (
                    <div className="text-center py-4 text-[var(--text-secondary)]">
                      {t('streams.noTargetsInGroup')}
                    </div>
                  )}
                </div>
              </OutputGroup>
            );
          })}

          <div
            className="flex justify-end border-t border-[var(--border-muted)]"
            style={{ gap: '12px', paddingTop: '16px' }}
          >
            <Button variant="outline" onClick={() => onNavigate('encoder')}>
              <Settings2 className="w-4 h-4" />
              {t('streams.configure')}
            </Button>
            {isStreaming ? (
              <Button variant="destructive" onClick={handleStopAll}>
                <Square className="w-4 h-4" />
                {t('streams.stopAllStreams')}
              </Button>
            ) : (
              <Button onClick={handleStartAll} disabled={isConnecting || isValidating}>
                <Play className="w-4 h-4" />
                {isValidating
                  ? t('streams.validating')
                  : isConnecting
                    ? t('streams.connecting')
                    : t('streams.startAllStreams')}
              </Button>
            )}
          </div>
        </CardBody>
      </Card>
    </div>
  );
}
