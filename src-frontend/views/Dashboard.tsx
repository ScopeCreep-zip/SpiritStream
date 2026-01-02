import { Radio, Activity, AlertTriangle, Clock, Monitor, Gauge, Target as TargetIcon } from 'lucide-react';
import { StatsRow } from '@/components/dashboard/StatsRow';
import { StatBox } from '@/components/dashboard/StatBox';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Grid } from '@/components/ui/Grid';
import { ProfileCard } from '@/components/dashboard/ProfileCard';
import { StreamCard } from '@/components/dashboard/StreamCard';
import { Button } from '@/components/ui/Button';
import { useProfileStore } from '@/stores/profileStore';
import { useStreamStore } from '@/stores/streamStore';
import type { Platform } from '@/types/profile';

export function Dashboard() {
  const { current: currentProfile, loading, error } = useProfileStore();
  const { isStreaming, stats, uptime, globalStatus } = useStreamStore();

  // Format uptime as HH:MM:SS
  const formatUptime = (seconds: number): string => {
    const hrs = Math.floor(seconds / 3600);
    const mins = Math.floor((seconds % 3600) / 60);
    const secs = seconds % 60;
    return `${hrs.toString().padStart(2, '0')}:${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
  };

  // Get all stream targets from current profile
  const getAllTargets = () => {
    if (!currentProfile) return [];
    return currentProfile.outputGroups.flatMap(group =>
      group.streamTargets.map(target => ({
        ...target,
        groupId: group.id,
      }))
    );
  };

  const targets = getAllTargets();

  // Calculate active streams count
  const activeStreamsCount = isStreaming ? targets.length : 0;

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

  return (
    <>
      <StatsRow>
        <StatBox
          icon={<Radio className="w-5 h-5" />}
          label="Active Streams"
          value={activeStreamsCount}
          change={isStreaming ? 'Streaming' : 'Ready to start'}
          changeType={isStreaming ? 'positive' : 'neutral'}
        />
        <StatBox
          icon={<Activity className="w-5 h-5" />}
          label="Total Bitrate"
          value={stats.totalBitrate > 0 ? `${stats.totalBitrate} kbps` : '0 kbps'}
          change={isStreaming ? 'Active' : 'No active streams'}
        />
        <StatBox
          icon={<AlertTriangle className="w-5 h-5" />}
          label="Dropped Frames"
          value={stats.droppedFrames}
          change={stats.droppedFrames === 0 ? 'No issues' : 'Check connection'}
          changeType={stats.droppedFrames === 0 ? 'positive' : 'neutral'}
        />
        <StatBox
          icon={<Clock className="w-5 h-5" />}
          label="Uptime"
          value={formatUptime(uptime)}
          change={isStreaming ? 'Live' : 'Not streaming'}
        />
      </StatsRow>

      <Grid cols={2} style={{ marginBottom: '24px' }}>
        <Card>
          <CardHeader>
            <div>
              <CardTitle>Active Profile</CardTitle>
              <CardDescription>Currently selected streaming configuration</CardDescription>
            </div>
            <Button variant="ghost" size="sm">Change</Button>
          </CardHeader>
          <CardBody>
            {currentProfile ? (
              <ProfileCard
                name={currentProfile.name}
                meta={[
                  { icon: <Monitor className="w-4 h-4" />, label: currentProfile.outputGroups[0]?.resolution || 'N/A' },
                  { icon: <Gauge className="w-4 h-4" />, label: `${currentProfile.outputGroups[0]?.videoBitrate || 0} kbps` },
                  { icon: <TargetIcon className="w-4 h-4" />, label: `${targets.length} targets` },
                ]}
                active
              />
            ) : (
              <div className="text-center py-8 text-[var(--text-secondary)]">
                <p>No profile selected</p>
                <Button variant="primary" className="mt-4">Create Profile</Button>
              </div>
            )}
          </CardBody>
        </Card>

        <Card>
          <CardHeader>
            <div>
              <CardTitle>Quick Actions</CardTitle>
              <CardDescription>Common streaming operations</CardDescription>
            </div>
          </CardHeader>
          <CardBody>
            <Grid cols={2} gap="sm">
              <Button variant="outline" className="justify-start">
                New Profile
              </Button>
              <Button variant="outline" className="justify-start">
                Import Profile
              </Button>
              <Button variant="outline" className="justify-start">
                Add Target
              </Button>
              <Button variant="outline" className="justify-start">
                Test Stream
              </Button>
            </Grid>
          </CardBody>
        </Card>
      </Grid>

      <Card>
        <CardHeader>
          <div>
            <CardTitle>Stream Targets</CardTitle>
            <CardDescription>Connected streaming platforms</CardDescription>
          </div>
          <Button variant="ghost" size="sm">Manage</Button>
        </CardHeader>
        <CardBody>
          {targets.length > 0 ? (
            <Grid cols={3}>
              {targets.map((target) => (
                <StreamCard
                  key={target.id}
                  platform={target.platform as Platform}
                  name={target.name}
                  status={globalStatus === 'live' ? 'live' : 'offline'}
                  stats={[
                    { label: 'Viewers', value: stats.targetStats[target.id]?.viewers || 0 },
                    { label: 'Bitrate', value: stats.targetStats[target.id]?.bitrate || '--' },
                    { label: 'FPS', value: stats.targetStats[target.id]?.fps || '--' },
                  ]}
                />
              ))}
            </Grid>
          ) : (
            <div className="text-center py-8 text-[var(--text-secondary)]">
              <p>No stream targets configured</p>
              <Button variant="outline" className="mt-4">Add Target</Button>
            </div>
          )}
        </CardBody>
      </Card>
    </>
  );
}
