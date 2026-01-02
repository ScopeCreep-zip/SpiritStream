import { Plus, Monitor, Gauge, Target } from 'lucide-react';
import { Grid } from '@/components/ui/Grid';
import { ProfileCard } from '@/components/dashboard/ProfileCard';
import { Button } from '@/components/ui/Button';
import { Card, CardBody } from '@/components/ui/Card';
import { useProfileStore } from '@/stores/profileStore';

export function Profiles() {
  const { profiles, current, loading, error, selectProfile, duplicateProfile } = useProfileStore();

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-[var(--text-secondary)]">Loading profiles...</div>
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

  // Empty state
  if (profiles.length === 0) {
    return (
      <Card>
        <CardBody>
          <div className="text-center" style={{ padding: '48px 0' }}>
            <div className="w-16 h-16 mx-auto rounded-full bg-[var(--primary-subtle)] flex items-center justify-center" style={{ marginBottom: '16px' }}>
              <Plus className="w-8 h-8 text-[var(--primary)]" />
            </div>
            <h3 className="text-lg font-semibold text-[var(--text-primary)]" style={{ marginBottom: '8px' }}>
              No Profiles Yet
            </h3>
            <p className="text-[var(--text-secondary)] max-w-md mx-auto" style={{ marginBottom: '24px' }}>
              Create your first streaming profile to get started. Profiles contain your encoder settings and stream targets.
            </p>
            <Button>
              <Plus className="w-4 h-4" />
              Create Your First Profile
            </Button>
          </div>
        </CardBody>
      </Card>
    );
  }

  return (
    <Grid cols={3}>
      {profiles.map((profile) => (
        <ProfileCard
          key={profile.id}
          name={profile.name}
          meta={[
            { icon: <Monitor className="w-4 h-4" />, label: profile.resolution },
            { icon: <Gauge className="w-4 h-4" />, label: `${profile.bitrate} kbps` },
            { icon: <Target className="w-4 h-4" />, label: `${profile.targetCount} targets` },
          ]}
          active={current?.id === profile.id}
          onClick={() => selectProfile(profile.id)}
          actions={
            <div className="flex" style={{ gap: '4px' }}>
              <Button
                variant="ghost"
                size="sm"
                onClick={(e) => {
                  e.stopPropagation();
                  alert('Edit modal not yet implemented');
                }}
              >
                Edit
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={(e) => {
                  e.stopPropagation();
                  duplicateProfile(profile.id);
                }}
              >
                Duplicate
              </Button>
            </div>
          }
        />
      ))}

      {/* Add New Profile Card */}
      <Card className="border-2 border-dashed border-[var(--border-default)] hover:border-[var(--primary)] transition-colors cursor-pointer">
        <CardBody className="flex flex-col items-center justify-center" style={{ padding: '48px 24px' }}>
          <div className="w-12 h-12 rounded-full bg-[var(--primary-subtle)] flex items-center justify-center" style={{ marginBottom: '12px' }}>
            <Plus className="w-6 h-6 text-[var(--primary)]" />
          </div>
          <span className="text-sm font-medium text-[var(--text-secondary)]">
            Create New Profile
          </span>
        </CardBody>
      </Card>
    </Grid>
  );
}
