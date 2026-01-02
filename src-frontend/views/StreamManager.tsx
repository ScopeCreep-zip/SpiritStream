import { useEffect, useRef } from 'react';
import { Play, Square, Settings2, Activity, Gauge, Clock } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Alert } from '@/components/ui/Alert';
import { Toggle } from '@/components/ui/Toggle';
import { OutputGroup } from '@/components/stream/OutputGroup';
import { PlatformIcon } from '@/components/stream/PlatformIcon';
import { useProfileStore } from '@/stores/profileStore';
import { useStreamStore } from '@/stores/streamStore';
import { formatUptime, formatBitrate } from '@/hooks/useStreamStats';
import { toast } from '@/hooks/useToast';
import type { View } from '@/App';
import type { Platform } from '@/types/profile';

interface StreamManagerProps {
  onNavigate: (view: View) => void;
}

export function StreamManager({ onNavigate }: StreamManagerProps) {
  console.log('[StreamManager] Component rendering');

  // Debug: Use ref to attach native DOM listener
  const testButtonRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    // Native DOM click handler (bypasses React)
    const btn = testButtonRef.current;
    if (btn) {
      const handler = () => {
        alert('NATIVE DOM CLICK WORKS!');
        console.log('[StreamManager] Native DOM click fired');
      };
      btn.addEventListener('click', handler);
      console.log('[StreamManager] Native click listener attached to test button');
      return () => btn.removeEventListener('click', handler);
    }
  }, []);

  useEffect(() => {
    const handleGlobalClick = (e: MouseEvent) => {
      console.log('[StreamManager] Global click detected:', e.target);
    };
    document.addEventListener('click', handleGlobalClick);
    console.log('[StreamManager] Component mounted, global click listener attached');
    return () => document.removeEventListener('click', handleGlobalClick);
  }, []);

  const { current, loading, error } = useProfileStore();
  const {
    isStreaming,
    activeGroups,
    enabledTargets,
    globalStatus,
    groupStats,
    error: streamError,
    startAllGroups,
    stopAllGroups,
    setTargetEnabled,
  } = useStreamStore();

  // Enable all targets by default when profile changes
  useEffect(() => {
    if (current) {
      const allTargetIds = current.outputGroups.flatMap(g =>
        g.streamTargets.map(t => t.id)
      );
      // Only add targets that aren't already in enabledTargets
      allTargetIds.forEach(id => {
        if (!enabledTargets.has(id)) {
          setTargetEnabled(id, true);
        }
      });
    }
  }, [current?.id]); // Re-run when profile changes

  const handleStartAll = async () => {
    console.log('[StreamManager] handleStartAll called');
    console.log('[StreamManager] current profile:', current);

    if (!current) {
      console.log('[StreamManager] No current profile');
      return;
    }

    // Validate incoming URL
    if (!current.incomingUrl || current.incomingUrl.trim() === '') {
      console.log('[StreamManager] Missing incoming URL');
      toast.error('No incoming URL configured. Set an RTMP source URL in your profile.');
      return;
    }

    // Validate we have stream targets
    const hasTargets = current.outputGroups.some(g => g.streamTargets.length > 0);
    if (!hasTargets) {
      console.log('[StreamManager] No stream targets');
      toast.error('No stream targets configured. Add at least one destination.');
      return;
    }

    console.log('[StreamManager] Enabled targets:', [...enabledTargets]);
    console.log('[StreamManager] Output groups:', current.outputGroups);
    console.log('[StreamManager] Incoming URL:', current.incomingUrl);

    try {
      console.log('[StreamManager] Calling startAllGroups...');
      await startAllGroups(current.outputGroups, current.incomingUrl);
      console.log('[StreamManager] startAllGroups completed successfully');
      toast.success('Streaming started');
    } catch (err) {
      console.error('[StreamManager] startAllGroups failed:', err);
      toast.error(`Failed to start streaming: ${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const handleStopAll = async () => {
    try {
      await stopAllGroups();
      toast.info('Streaming stopped');
    } catch (err) {
      toast.error(`Failed to stop streaming: ${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const isConnecting = globalStatus === 'connecting';

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-[var(--text-secondary)]">Loading...</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-[var(--error-text)]">Error: {error}</div>
      </div>
    );
  }

  if (!current) {
    return (
      <Card>
        <CardBody>
          <div className="text-center py-12">
            <p className="text-[var(--text-secondary)]">
              Please select a profile first to manage streams.
            </p>
          </div>
        </CardBody>
      </Card>
    );
  }

  const outputGroups = current.outputGroups;

  if (outputGroups.length === 0) {
    return (
      <Alert variant="warning" title="No Output Groups">
        You need to create at least one output group with stream targets before you can start streaming.
      </Alert>
    );
  }

  // Get status for each group
  const getGroupStatus = (groupId: string): 'live' | 'offline' | 'error' => {
    if (activeGroups.has(groupId)) return 'live';
    return 'offline';
  };

  // Get group info string
  const getGroupInfo = (group: typeof outputGroups[0]): string => {
    const targetCount = group.streamTargets.length;
    return `${targetCount} target${targetCount !== 1 ? 's' : ''} • ${group.resolution} • ${group.videoBitrate} kbps`;
  };

  return (
    <div className="flex flex-col" style={{ gap: '24px' }}>
      {streamError && (
        <Alert variant="error" title="Stream Error">
          {streamError}
        </Alert>
      )}

      {!isStreaming && !streamError && (
        <Alert variant="info" title="Ready to Stream">
          Configure your output groups below and click "Start All Streams" when ready.
        </Alert>
      )}

      <Card>
        <CardHeader>
          <div>
            <CardTitle>Stream Control</CardTitle>
            <CardDescription>
              Manage your active streams and target configurations
            </CardDescription>
          </div>
        </CardHeader>
        <CardBody style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
          {outputGroups.map((group) => {
            const stats = groupStats[group.id];
            const isGroupActive = activeGroups.has(group.id);

            return (
            <OutputGroup
              key={group.id}
              name={group.name || `Output Group`}
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
                      <span className="text-xs">FPS</span>
                    </div>
                    <div className="text-lg font-semibold text-[var(--text-primary)]">
                      {stats.fps.toFixed(1)}
                    </div>
                  </div>
                  <div className="text-center">
                    <div className="flex items-center justify-center gap-1 text-[var(--text-tertiary)] mb-1">
                      <Gauge className="w-3 h-3" />
                      <span className="text-xs">Bitrate</span>
                    </div>
                    <div className="text-lg font-semibold text-[var(--text-primary)]">
                      {formatBitrate(stats.bitrate)}
                    </div>
                  </div>
                  <div className="text-center">
                    <div className="flex items-center justify-center gap-1 text-[var(--text-tertiary)] mb-1">
                      <Clock className="w-3 h-3" />
                      <span className="text-xs">Uptime</span>
                    </div>
                    <div className="text-lg font-semibold text-[var(--text-primary)]">
                      {formatUptime(stats.uptime)}
                    </div>
                  </div>
                  <div className="text-center">
                    <div className="text-xs text-[var(--text-tertiary)] mb-1">Speed</div>
                    <div className="text-lg font-semibold text-[var(--text-primary)]">
                      {stats.speed.toFixed(2)}x
                    </div>
                  </div>
                </div>
              )}

              <div className="flex flex-col border-t border-[var(--border-muted)]" style={{ gap: '12px', paddingTop: '12px' }}>
                {group.streamTargets.map((target) => (
                  <div
                    key={target.id}
                    className="flex items-center justify-between rounded-lg bg-[var(--bg-surface)] border border-[var(--border-default)]"
                    style={{ padding: '12px' }}
                  >
                    <div className="flex items-center" style={{ gap: '12px' }}>
                      <PlatformIcon platform={target.platform as Platform} size="sm" />
                      <div>
                        <div className="font-medium text-sm text-[var(--text-primary)]">
                          {target.name}
                        </div>
                        <div className="text-xs text-[var(--text-tertiary)]">
                          {target.url}
                        </div>
                      </div>
                    </div>
                    <Toggle
                      checked={enabledTargets.has(target.id)}
                      onChange={(checked) => setTargetEnabled(target.id, checked)}
                      disabled={isStreaming}
                    />
                  </div>
                ))}

                {group.streamTargets.length === 0 && (
                  <div className="text-center py-4 text-[var(--text-secondary)]">
                    No stream targets in this group
                  </div>
                )}
              </div>
            </OutputGroup>
          );
          })}

          <div className="flex justify-end border-t border-[var(--border-muted)]" style={{ gap: '12px', paddingTop: '16px' }}>
            <button
              type="button"
              onClick={() => alert('TEST CLICK')}
              style={{ padding: '10px 20px', background: 'red', color: 'white', cursor: 'pointer' }}
            >
              TEST BUTTON
            </button>
            <Button variant="outline" onClick={() => onNavigate('encoder')}>
              <Settings2 className="w-4 h-4" />
              Configure
            </Button>
            {isStreaming ? (
              <Button variant="destructive" onClick={handleStopAll}>
                <Square className="w-4 h-4" />
                Stop All Streams
              </Button>
            ) : (
              <Button onClick={() => { console.log('BUTTON CLICKED!'); handleStartAll(); }} disabled={isConnecting}>
                <Play className="w-4 h-4" />
                {isConnecting ? 'Connecting...' : 'Start All Streams'}
              </Button>
            )}
          </div>
        </CardBody>
      </Card>
    </div>
  );
}
