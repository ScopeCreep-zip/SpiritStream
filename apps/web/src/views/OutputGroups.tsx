import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useShallow } from 'zustand/shallow';
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

export default function OutputGroups() {
  const { t } = useTranslation();
  const { current, loading, error, updateOutputGroup, removeOutputGroup, addOutputGroup } =
    useProfileStore(useShallow((state) => ({
      current: state.current,
      loading: state.loading,
      error: state.error,
      updateOutputGroup: state.updateOutputGroup,
      removeOutputGroup: state.removeOutputGroup,
      addOutputGroup: state.addOutputGroup,
    })));
  const { activeGroups } = useStreamStore(useShallow((state) => ({
    activeGroups: state.activeGroups,
  })));
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

  const outputGroups = current?.outputGroups ?? [];

  // Get status for each group
  const getGroupStatus = useCallback(
    (groupId: string): 'live' | 'connecting' | 'offline' | 'error' => {
      if (activeGroups.has(groupId)) return 'live';
      return 'offline';
    },
    [activeGroups]
  );

  const duplicateGroup = useCallback(
    (groupId: string) => {
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
    },
    [outputGroups, t, addOutputGroup]
  );

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

  if (outputGroups.length === 0) {
    return (
      <>
        <Card>
          <CardBody>
            <div className="text-center py-12">
              <div className="w-16 h-16 mx-auto rounded-full bg-[var(--primary-subtle)] flex items-center justify-center mb-4">
                <Plus className="w-8 h-8 text-[var(--primary)]" />
              </div>
              <h3 className="text-lg font-semibold text-[var(--text-primary)] mb-2">
                {t('outputs.noOutputGroups')}
              </h3>
              <p className="text-[var(--text-secondary)] max-w-md mx-auto mb-6">
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

  return (
    <div className="flex flex-col gap-4">
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
        <CardBody className="flex items-center justify-center py-8 px-6">
          <Button variant="ghost">
            <Plus className="w-5 h-5 mr-2" />
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
