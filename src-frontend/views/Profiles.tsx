import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Monitor, Gauge, Target } from 'lucide-react';
import { Grid } from '@/components/ui/Grid';
import { ProfileCard } from '@/components/dashboard/ProfileCard';
import { Button } from '@/components/ui/Button';
import { Card, CardBody } from '@/components/ui/Card';
import { ProfileModal } from '@/components/modals';
import { useProfileStore } from '@/stores/profileStore';

export function Profiles() {
  const { t } = useTranslation();
  const { profiles, current, loading, error, selectProfile, duplicateProfile } = useProfileStore();
  const [editModalOpen, setEditModalOpen] = useState(false);
  const [createModalOpen, setCreateModalOpen] = useState(false);

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-[var(--text-secondary)]">{t('profiles.loadingProfiles')}</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-[var(--error-text)]">{t('common.error')}: {error}</div>
      </div>
    );
  }

  // Empty state
  if (profiles.length === 0) {
    return (
      <>
        <Card>
          <CardBody>
            <div className="text-center" style={{ padding: '48px 0' }}>
              <div className="w-16 h-16 mx-auto rounded-full bg-[var(--primary-subtle)] flex items-center justify-center" style={{ marginBottom: '16px' }}>
                <Plus className="w-8 h-8 text-[var(--primary)]" />
              </div>
              <h3 className="text-lg font-semibold text-[var(--text-primary)]" style={{ marginBottom: '8px' }}>
                {t('profiles.noProfilesYet')}
              </h3>
              <p className="text-[var(--text-secondary)] max-w-md mx-auto" style={{ marginBottom: '24px' }}>
                {t('profiles.noProfilesDescription')}
              </p>
              <Button onClick={() => setCreateModalOpen(true)}>
                <Plus className="w-4 h-4" />
                {t('profiles.createFirstProfile')}
              </Button>
            </div>
          </CardBody>
        </Card>

        <ProfileModal
          open={createModalOpen}
          onClose={() => setCreateModalOpen(false)}
          mode="create"
        />
      </>
    );
  }

  const handleEdit = async (profileName: string) => {
    // Load the profile first, then open edit modal
    await selectProfile(profileName);
    setEditModalOpen(true);
  };

  return (
    <>
      <Grid cols={3}>
        {profiles.map((profile) => (
          <ProfileCard
            key={profile.id}
            name={profile.name}
            meta={[
              { icon: <Monitor className="w-4 h-4" />, label: profile.resolution },
              { icon: <Gauge className="w-4 h-4" />, label: `${profile.bitrate} kbps` },
              { icon: <Target className="w-4 h-4" />, label: t('profiles.targetsCount', { count: profile.targetCount }) },
            ]}
            active={current?.id === profile.id}
            onClick={() => selectProfile(profile.name)}
            actions={
              <div className="flex" style={{ gap: '4px' }}>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={(e) => {
                    e.stopPropagation();
                    handleEdit(profile.name);
                  }}
                >
                  {t('common.edit')}
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={(e) => {
                    e.stopPropagation();
                    duplicateProfile(profile.name);
                  }}
                >
                  {t('common.duplicate')}
                </Button>
              </div>
            }
          />
        ))}

        {/* Add New Profile Card */}
        <Card
          className="border-2 border-dashed border-[var(--border-default)] hover:border-[var(--primary)] transition-colors cursor-pointer"
          onClick={() => setCreateModalOpen(true)}
        >
          <CardBody className="flex flex-col items-center justify-center" style={{ padding: '48px 24px' }}>
            <div className="w-12 h-12 rounded-full bg-[var(--primary-subtle)] flex items-center justify-center" style={{ marginBottom: '12px' }}>
              <Plus className="w-6 h-6 text-[var(--primary)]" />
            </div>
            <span className="text-sm font-medium text-[var(--text-secondary)]">
              {t('profiles.createNewProfile')}
            </span>
          </CardBody>
        </Card>
      </Grid>

      {/* Create Profile Modal */}
      <ProfileModal
        open={createModalOpen}
        onClose={() => setCreateModalOpen(false)}
        mode="create"
      />

      {/* Edit Profile Modal */}
      <ProfileModal
        open={editModalOpen}
        onClose={() => setEditModalOpen(false)}
        mode="edit"
        profile={current || undefined}
      />
    </>
  );
}
