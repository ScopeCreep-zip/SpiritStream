import { Play, Square, Settings2 } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Alert } from '@/components/ui/Alert';
import { Toggle } from '@/components/ui/Toggle';
import { OutputGroup } from '@/components/stream/OutputGroup';
import { PlatformIcon } from '@/components/stream/PlatformIcon';
import { useProfileStore } from '@/stores/profileStore';
import { useStreamStore } from '@/stores/streamStore';
import type { Platform } from '@/types/profile';

export function StreamManager() {
  const { current, loading, error } = useProfileStore();
  const {
    isStreaming,
    activeGroups,
    enabledTargets,
    setIsStreaming,
    setActiveGroup,
    setTargetEnabled,
  } = useStreamStore();

  const handleStartAll = () => {
    if (!current) return;
    setIsStreaming(true);
    current.outputGroups.forEach(group => {
      setActiveGroup(group.id, true);
    });
    // TODO: Call Tauri to actually start streams
  };

  const handleStopAll = () => {
    if (!current) return;
    setIsStreaming(false);
    current.outputGroups.forEach(group => {
      setActiveGroup(group.id, false);
    });
    // TODO: Call Tauri to actually stop streams
  };

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
      {!isStreaming && (
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
          {outputGroups.map((group) => (
            <OutputGroup
              key={group.id}
              name={group.name || `Output Group`}
              info={getGroupInfo(group)}
              status={getGroupStatus(group.id)}
              defaultExpanded={false}
            >
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
          ))}

          <div className="flex justify-end border-t border-[var(--border-muted)]" style={{ gap: '12px', paddingTop: '16px' }}>
            <Button variant="outline">
              <Settings2 className="w-4 h-4" />
              Configure
            </Button>
            {isStreaming ? (
              <Button variant="destructive" onClick={handleStopAll}>
                <Square className="w-4 h-4" />
                Stop All Streams
              </Button>
            ) : (
              <Button onClick={handleStartAll}>
                <Play className="w-4 h-4" />
                Start All Streams
              </Button>
            )}
          </div>
        </CardBody>
      </Card>
    </div>
  );
}
