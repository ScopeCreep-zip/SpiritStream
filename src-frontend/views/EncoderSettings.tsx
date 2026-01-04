import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Cpu } from 'lucide-react';
import { Card, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { EncoderCard } from '@/components/encoder';
import { OutputGroupModal } from '@/components/modals';
import { useProfileStore } from '@/stores/profileStore';
import { useStreamStore } from '@/stores/streamStore';
import type { OutputGroup } from '@/types/profile';

export function EncoderSettings() {
  const { t } = useTranslation();
  const tDynamic = t as (key: string, options?: { defaultValue?: string }) => string;

  const { current, loading, error, addOutputGroup, removeOutputGroup } = useProfileStore();
  const { activeGroups } = useStreamStore();

  const [createModalOpen, setCreateModalOpen] = useState(false);
  const [editModalOpen, setEditModalOpen] = useState(false);
  const [editingGroup, setEditingGroup] = useState<OutputGroup | null>(null);

  // Loading state
  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-[var(--text-secondary)]">{t('common.loading')}</div>
      </div>
    );
  }

  // Error state
  if (error) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-[var(--error-text)]">
          {t('common.error')}: {error}
        </div>
      </div>
    );
  }

  // No profile selected
  if (!current) {
    return (
      <Card>
        <CardBody>
          <div className="text-center py-12">
            <p className="text-[var(--text-secondary)]">
              {tDynamic('encoder.selectProfileFirst', { defaultValue: 'Select a profile first' })}
            </p>
          </div>
        </CardBody>
      </Card>
    );
  }

  const outputGroups = current.outputGroups;

  // Get status for each group
  const getGroupStatus = (groupId: string): 'live' | 'connecting' | 'offline' | 'error' => {
    if (activeGroups.has(groupId)) return 'live';
    return 'offline';
  };

  // Duplicate a group
  const duplicateGroup = (group: OutputGroup) => {
    const newGroup: OutputGroup = {
      ...group,
      id: crypto.randomUUID(),
      name: `${group.name} ${tDynamic('common.copySuffix', { defaultValue: '(Copy)' })}`,
      isDefault: false, // Duplicates are never default groups
      // Deep clone nested objects
      video: { ...group.video },
      audio: { ...group.audio },
      container: { ...group.container },
      streamTargets: group.streamTargets.map((t) => ({ ...t, id: crypto.randomUUID() })),
    };
    addOutputGroup(newGroup);
  };

  // Open edit modal
  const openEditModal = (group: OutputGroup) => {
    setEditingGroup(group);
    setEditModalOpen(true);
  };

  // Close edit modal
  const closeEditModal = () => {
    setEditingGroup(null);
    setEditModalOpen(false);
  };

  // Empty state
  if (outputGroups.length === 0) {
    return (
      <>
        <Card>
          <CardBody>
            <div className="text-center" style={{ padding: '48px 0' }}>
              <div
                className="w-16 h-16 mx-auto rounded-full bg-[var(--primary-subtle)] flex items-center justify-center"
                style={{ marginBottom: '16px' }}
              >
                <Cpu className="w-8 h-8 text-[var(--primary)]" />
              </div>
              <h3
                className="text-lg font-semibold text-[var(--text-primary)]"
                style={{ marginBottom: '8px' }}
              >
                {tDynamic('encoder.noEncoders', { defaultValue: 'No Encoder Configurations' })}
              </h3>
              <p
                className="text-[var(--text-secondary)] max-w-md mx-auto"
                style={{ marginBottom: '24px' }}
              >
                {tDynamic('encoder.noEncodersDescription', {
                  defaultValue:
                    'Create your first encoder configuration to define video and audio encoding settings for your streams.',
                })}
              </p>
              <Button onClick={() => setCreateModalOpen(true)}>
                <Plus className="w-4 h-4" />
                {tDynamic('encoder.addEncoder', { defaultValue: 'Add Encoder' })}
              </Button>
            </div>
          </CardBody>
        </Card>

        {/* Create Modal */}
        <OutputGroupModal
          open={createModalOpen}
          onClose={() => setCreateModalOpen(false)}
          mode="create"
        />
      </>
    );
  }

  return (
    <div className="flex flex-col" style={{ gap: '16px' }}>
      {/* Encoder Cards */}
      {outputGroups.map((group) => (
        <EncoderCard
          key={group.id}
          group={group}
          status={getGroupStatus(group.id)}
          onEdit={group.isDefault ? undefined : () => openEditModal(group)}
          onDuplicate={() => duplicateGroup(group)}
          onRemove={() => removeOutputGroup(group.id)}
        />
      ))}

      {/* Add New Encoder Card */}
      <Card
        className="border-2 border-dashed border-[var(--border-default)] hover:border-[var(--primary)] transition-colors cursor-pointer"
        onClick={() => setCreateModalOpen(true)}
      >
        <CardBody className="flex items-center justify-center" style={{ padding: '32px 24px' }}>
          <Button variant="ghost">
            <Plus className="w-5 h-5" style={{ marginRight: '8px' }} />
            {tDynamic('encoder.addEncoder', { defaultValue: 'Add Encoder' })}
          </Button>
        </CardBody>
      </Card>

      {/* Create Modal */}
      <OutputGroupModal
        open={createModalOpen}
        onClose={() => setCreateModalOpen(false)}
        mode="create"
      />

      {/* Edit Modal */}
      <OutputGroupModal
        open={editModalOpen}
        onClose={closeEditModal}
        mode="edit"
        group={editingGroup || undefined}
      />
    </div>
  );
}
