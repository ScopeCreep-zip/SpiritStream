import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Radio,
  Activity,
  AlertTriangle,
  Clock,
  Monitor,
  Gauge,
  Target as TargetIcon,
  Upload,
  Loader2,
} from 'lucide-react';
import { StatsRow } from '@/components/dashboard/StatsRow';
import { StatBox } from '@/components/dashboard/StatBox';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Grid } from '@/components/ui/Grid';
import { ProfileCard } from '@/components/dashboard/ProfileCard';
import { StreamCard } from '@/components/dashboard/StreamCard';
import { OutputGroupCard } from '@/components/stream/OutputGroupCard';
import { EncoderCard } from '@/components/encoder/EncoderCard';
import { Button } from '@/components/ui/Button';
import { useProfileStore } from '@/stores/profileStore';
import { useStreamStore } from '@/stores/streamStore';
import { formatUptime, formatBitrate } from '@/hooks/useStreamStats';
import { toast } from '@/hooks/useToast';
import { api, dialogs } from '@/lib/backend';
import { validateStreamConfig, displayValidationIssues } from '@/lib/streamValidation';
import type { View } from '@/App';
import type { Profile, OutputGroup } from '@/types/profile';

interface DashboardProps {
  onNavigate: (view: View) => void;
  onOpenProfileModal: () => void;
  onOpenTargetModal: () => void;
}

export function Dashboard({ onNavigate, onOpenProfileModal, onOpenTargetModal }: DashboardProps) {
  const { t } = useTranslation();
  const {
    current: currentProfile,
    loading,
    error,
    loadProfiles,
    addOutputGroup,
    removeOutputGroup,
  } = useProfileStore();
  const { isStreaming, stats, uptime, globalStatus, activeStreamCount, activeGroups, enabledTargets } =
    useStreamStore();
  const [isTesting, setIsTesting] = useState(false);

  // Get output groups from current profile
  const outputGroups = currentProfile?.outputGroups ?? [];

  // Handle duplicating an output group
  const handleDuplicateGroup = async (group: OutputGroup) => {
    const duplicatedGroup: OutputGroup = {
      ...group,
      id: crypto.randomUUID(),
      name: `${group.name} (Copy)`,
      streamTargets: group.streamTargets.map((target) => ({
        ...target,
        id: crypto.randomUUID(),
      })),
    };
    await addOutputGroup(duplicatedGroup);
    toast.success(t('toast.groupDuplicated', { name: group.name }));
  };

  // Handle removing an output group
  const handleRemoveGroup = async (groupId: string) => {
    await removeOutputGroup(groupId);
    toast.success(t('toast.groupRemoved'));
  };

  // Import profile from JSON file
  const handleImportProfile = async () => {
    try {
      const selected = await dialogs.openTextFile({
        multiple: false,
        filters: [{ name: 'Profile', extensions: ['json'] }],
      });

      if (!selected) return;

      const profile = JSON.parse(selected.content) as Profile;

      // Validate basic structure
      if (!profile.name || !profile.outputGroups) {
        throw new Error(t('errors.invalidProfileFormat'));
      }

      // Save imported profile
      await api.profile.save(profile);
      await loadProfiles();
      toast.success(t('toast.profileImported', { name: profile.name }));
    } catch (err) {
      toast.error(
        t('toast.importFailed', { error: err instanceof Error ? err.message : t('errors.unknown') })
      );
    }
  };

  // Test stream configuration without actually streaming
  const handleTestStream = async () => {
    if (!currentProfile) {
      toast.error(t('errors.noProfileSelected'));
      return;
    }

    setIsTesting(true);
    toast.info(t('toast.testingConfig'));

    try {
      // Run comprehensive validation (test ALL targets, not just enabled)
      const result = await validateStreamConfig(currentProfile, {
        checkFfmpeg: true,
        checkEnabledTargetsOnly: false,
      });

      if (result.valid) {
        toast.success(t('toast.testPassed'));
      } else {
        displayValidationIssues(result.issues, toast);
      }
    } catch (err) {
      toast.error(
        t('toast.testFailed', { error: err instanceof Error ? err.message : t('errors.unknown') })
      );
    } finally {
      setIsTesting(false);
    }
  };

  // Get all stream targets from current profile
  const getAllTargets = () => {
    if (!currentProfile) return [];
    return currentProfile.outputGroups.flatMap((group) =>
      group.streamTargets.map((target) => ({
        id: target.id,
        name: target.name,
        service: target.service, // Using service (not platform) from new structure
        url: target.url,
        streamKey: target.streamKey,
        groupId: group.id,
      }))
    );
  };

  const targets = getAllTargets();

  const activeTargetCount = useMemo(() => {
    if (!currentProfile) return 0;

    return currentProfile.outputGroups.reduce((total, group) => {
      if (!activeGroups.has(group.id)) {
        return total;
      }
      const enabledCount = group.streamTargets.filter((t) => enabledTargets.has(t.id)).length;
      return total + enabledCount;
    }, 0);
  }, [currentProfile, activeGroups, enabledTargets]);

  // Use backend-verified active stream count, fallback to local calculation
  const displayActiveCount =
    activeTargetCount > 0
      ? activeTargetCount
      : activeStreamCount > 0
        ? activeStreamCount
        : isStreaming
          ? targets.length
          : 0;

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

  return (
    <>
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

      <Grid cols={2} style={{ marginBottom: '24px' }}>
        <Card>
          <CardHeader>
            <div>
              <CardTitle>{t('dashboard.activeProfile')}</CardTitle>
              <CardDescription>{t('dashboard.activeProfileDescription')}</CardDescription>
            </div>
            <Button variant="ghost" size="sm" onClick={() => onNavigate('profiles')}>
              {t('common.change')}
            </Button>
          </CardHeader>
          <CardBody>
            {currentProfile ? (
              <ProfileCard
                name={currentProfile.name}
                meta={[
                  {
                    icon: <Monitor className="w-4 h-4" />,
                    label: currentProfile.outputGroups[0]?.video
                      ? `${currentProfile.outputGroups[0].video.height}p${currentProfile.outputGroups[0].video.fps}`
                      : 'N/A',
                  },
                  {
                    icon: <Gauge className="w-4 h-4" />,
                    label: currentProfile.outputGroups[0]?.video?.bitrate || '0k',
                  },
                  {
                    icon: <TargetIcon className="w-4 h-4" />,
                    label: t('dashboard.targetsCount', { count: targets.length }),
                  },
                ]}
                services={[...new Set(targets.map((t) => t.service))]}
                active
              />
            ) : (
              <div className="text-center py-8 text-[var(--text-secondary)]">
                <p>{t('dashboard.noProfileSelected')}</p>
                <Button variant="primary" className="mt-4" onClick={onOpenProfileModal}>
                  {t('dashboard.createProfile')}
                </Button>
              </div>
            )}
          </CardBody>
        </Card>

        <Card>
          <CardHeader>
            <div>
              <CardTitle>{t('dashboard.quickActions')}</CardTitle>
              <CardDescription>{t('dashboard.quickActionsDescription')}</CardDescription>
            </div>
          </CardHeader>
          <CardBody>
            <Grid cols={2} gap="sm">
              <Button variant="outline" className="justify-start" onClick={onOpenProfileModal}>
                {t('profiles.newProfile')}
              </Button>
              <Button variant="outline" className="justify-start" onClick={handleImportProfile}>
                <Upload className="w-4 h-4" />
                {t('dashboard.importProfile')}
              </Button>
              <Button
                variant="outline"
                className="justify-start"
                onClick={onOpenTargetModal}
                disabled={!currentProfile}
              >
                {t('targets.addTarget')}
              </Button>
              <Button
                variant="outline"
                className="justify-start"
                onClick={handleTestStream}
                disabled={isTesting || isStreaming || !currentProfile}
              >
                {isTesting ? <Loader2 className="w-4 h-4 animate-spin" /> : null}
                {isTesting ? t('dashboard.testing') : t('dashboard.testStream')}
              </Button>
            </Grid>
          </CardBody>
        </Card>
      </Grid>

      <Card>
        <CardHeader>
          <div>
            <CardTitle>{t('dashboard.streamTargets')}</CardTitle>
            <CardDescription>{t('dashboard.streamTargetsDescription')}</CardDescription>
          </div>
          <Button variant="ghost" size="sm" onClick={() => onNavigate('targets')}>
            {t('common.manage')}
          </Button>
        </CardHeader>
        <CardBody>
          {targets.length > 0 ? (
            <Grid cols={3}>
              {targets.map((target) => (
                <StreamCard
                  key={target.id}
                  platform={target.service}
                  name={target.name}
                  status={globalStatus === 'live' ? 'live' : 'offline'}
                  stats={[
                    {
                      label: t('dashboard.viewers'),
                      value: stats.targetStats[target.id]?.viewers || 0,
                    },
                    {
                      label: t('dashboard.bitrate'),
                      value: stats.targetStats[target.id]?.bitrate || '--',
                    },
                    { label: t('dashboard.fps'), value: stats.targetStats[target.id]?.fps || '--' },
                  ]}
                />
              ))}
            </Grid>
          ) : (
            <div className="text-center py-8 text-[var(--text-secondary)]">
              <p>{t('dashboard.noStreamTargets')}</p>
              <Button
                variant="outline"
                className="mt-4"
                onClick={onOpenTargetModal}
                disabled={!currentProfile}
              >
                {t('targets.addTarget')}
              </Button>
            </div>
          )}
        </CardBody>
      </Card>

      {/* Output Groups Section */}
      <Card style={{ marginTop: '24px' }}>
        <CardHeader>
          <div>
            <CardTitle>{t('dashboard.outputGroups')}</CardTitle>
            <CardDescription>{t('dashboard.outputGroupsDescription')}</CardDescription>
          </div>
          <Button variant="ghost" size="sm" onClick={() => onNavigate('outputs')}>
            {t('common.manage')}
          </Button>
        </CardHeader>
        <CardBody>
          {outputGroups.length > 0 ? (
            <Grid cols={2}>
              {outputGroups.map((group, index) => (
                <OutputGroupCard
                  key={group.id}
                  group={group}
                  index={index}
                  encoders={{ video: [], audio: [] }}
                  status={globalStatus === 'live' ? 'live' : 'offline'}
                  onUpdate={() => {}}
                  onEdit={() => onNavigate('outputs')}
                  onDuplicate={() => handleDuplicateGroup(group)}
                  onRemove={() => handleRemoveGroup(group.id)}
                />
              ))}
            </Grid>
          ) : (
            <div className="text-center py-8 text-[var(--text-secondary)]">
              <p>{t('dashboard.noOutputGroups')}</p>
              <Button
                variant="outline"
                className="mt-4"
                onClick={() => onNavigate('outputs')}
                disabled={!currentProfile}
              >
                {t('outputs.newOutputGroup')}
              </Button>
            </div>
          )}
        </CardBody>
      </Card>

      {/* Encoder Settings Section */}
      <Card style={{ marginTop: '24px' }}>
        <CardHeader>
          <div>
            <CardTitle>{t('dashboard.encoderSettings')}</CardTitle>
            <CardDescription>{t('dashboard.encoderSettingsDescription')}</CardDescription>
          </div>
          <Button variant="ghost" size="sm" onClick={() => onNavigate('encoder')}>
            {t('common.manage')}
          </Button>
        </CardHeader>
        <CardBody>
          {outputGroups.length > 0 ? (
            <Grid cols={2}>
              {outputGroups.map((group) => (
                <EncoderCard
                  key={group.id}
                  group={group}
                  status={globalStatus === 'live' ? 'live' : 'offline'}
                  onEdit={() => onNavigate('encoder')}
                  onDuplicate={() => handleDuplicateGroup(group)}
                  onRemove={() => handleRemoveGroup(group.id)}
                />
              ))}
            </Grid>
          ) : (
            <div className="text-center py-8 text-[var(--text-secondary)]">
              <p>{t('dashboard.noEncoderSettings')}</p>
              <Button
                variant="outline"
                className="mt-4"
                onClick={() => onNavigate('encoder')}
                disabled={!currentProfile}
              >
                {t('encoder.addEncoder')}
              </Button>
            </div>
          )}
        </CardBody>
      </Card>
    </>
  );
}
