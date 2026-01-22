import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus } from 'lucide-react';
import { Card, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { OutputGroupCard } from '@/components/stream/OutputGroupCard';
import { OutputGroupModal, TargetModal } from '@/components/modals';
import { useProfileStore } from '@/stores/profileStore';
import { useStreamStore } from '@/stores/streamStore';
import { api } from '@/lib/backend';
import type { OutputGroup } from '@/types/profile';
import type { Encoders } from '@/types/stream';

export function OutputGroups() {
  const { t } = useTranslation();
  const { current, loading, error, updateOutputGroup, removeOutputGroup, addOutputGroup } =
    useProfileStore();
  const { activeGroups } = useStreamStore();
  const [createModalOpen, setCreateModalOpen] = useState(false);
  const [editModalOpen, setEditModalOpen] = useState(false);
  const [editingGroup, setEditingGroup] = useState<OutputGroup | null>(null);
  const [encoders, setEncoders] = useState<Encoders>({ video: ['libx264'], audio: ['aac'] });
  const [addTargetModalOpen, setAddTargetModalOpen] = useState(false);
  const [addTargetGroupId, setAddTargetGroupId] = useState<string>('');

  // Fetch available encoders from backend
  useEffect(() => {
    api.system
      .getEncoders()
      .then(setEncoders)
      .catch((err) => console.error('Failed to load encoders:', err));
  }, []);

  const openEditModal = (group: OutputGroup) => {
    setEditingGroup(group);
    setEditModalOpen(true);
  };

  const closeEditModal = () => {
    setEditingGroup(null);
    setEditModalOpen(false);
  };

  const openAddTargetModal = (groupId: string) => {
    setAddTargetGroupId(groupId);
    setAddTargetModalOpen(true);
  };

  const closeAddTargetModal = () => {
    setAddTargetGroupId('');
    setAddTargetModalOpen(false);
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
            <p className="text-[var(--text-secondary)]">{t('outputs.selectProfileFirst')}</p>
          </div>
        </CardBody>
      </Card>
    );
  }

  const outputGroups = current.outputGroups;

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
                <Plus className="w-8 h-8 text-[var(--primary)]" />
              </div>
              <h3
                className="text-lg font-semibold text-[var(--text-primary)]"
                style={{ marginBottom: '8px' }}
              >
                {t('outputs.noOutputGroups')}
              </h3>
              <p
                className="text-[var(--text-secondary)] max-w-md mx-auto"
                style={{ marginBottom: '24px' }}
              >
                {t('outputs.noOutputGroupsDescription')}
              </p>
              <Button onClick={() => setCreateModalOpen(true)}>
                <Plus className="w-4 h-4" />
                {t('outputs.createOutputGroup')}
              </Button>
            </div>
          </CardBody>
        </Card>

        {/* Create Modal for empty state */}
        <OutputGroupModal
          open={createModalOpen}
          onClose={() => setCreateModalOpen(false)}
          mode="create"
        />
      </>
    );
  }

  // Get status for each group
  const getGroupStatus = (groupId: string): 'live' | 'connecting' | 'offline' | 'error' => {
    if (activeGroups.has(groupId)) return 'live';
    return 'offline';
  };

  const duplicateGroup = (groupId: string) => {
    const group = outputGroups.find((g) => g.id === groupId);
    if (group) {
      const newGroup = {
        ...group,
        id: crypto.randomUUID(),
        name: `${group.name} ${t('common.copySuffix')}`,
        isDefault: false, // Duplicates are never default groups
      };
      addOutputGroup(newGroup);
    }
  };

  return (
    <div className="flex flex-col" style={{ gap: '16px' }}>
      {outputGroups.map((group, index) => (
        <OutputGroupCard
          key={group.id}
          group={group}
          index={index}
          encoders={encoders}
          status={getGroupStatus(group.id)}
          onUpdate={(updates) => updateOutputGroup(group.id, updates)}
          onRemove={() => removeOutputGroup(group.id)}
          onDuplicate={group.isDefault ? undefined : () => duplicateGroup(group.id)}
          onEdit={group.isDefault ? undefined : () => openEditModal(group)}
          onAddTarget={() => openAddTargetModal(group.id)}
        />
      ))}

      {/* Add New Group Button */}
      <Card
        className="border-2 border-dashed border-[var(--border-default)] hover:border-[var(--primary)] transition-colors cursor-pointer"
        onClick={() => setCreateModalOpen(true)}
      >
        <CardBody className="flex items-center justify-center" style={{ padding: '32px 24px' }}>
          <Button variant="ghost">
            <Plus className="w-5 h-5" style={{ marginRight: '8px' }} />
            {t('outputs.addOutputGroup')}
          </Button>
        </CardBody>
      </Card>

      {/* Create Output Group Modal */}
      <OutputGroupModal
        open={createModalOpen}
        onClose={() => setCreateModalOpen(false)}
        mode="create"
      />

      {/* Edit Output Group Modal */}
      <OutputGroupModal
        open={editModalOpen}
        onClose={closeEditModal}
        mode="edit"
        group={editingGroup || undefined}
      />

      {/* Add Target Modal */}
      <TargetModal
        open={addTargetModalOpen}
        onClose={closeAddTargetModal}
        mode="create"
        groupId={addTargetGroupId}
      />
    </div>
  );
}
