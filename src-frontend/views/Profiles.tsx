import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Monitor, Gauge, Target, Lock, Unlock, Trash2 } from 'lucide-react';
import { Grid } from '@/components/ui/Grid';
import { ProfileCard } from '@/components/dashboard/ProfileCard';
import { Button } from '@/components/ui/Button';
import { Card, CardBody } from '@/components/ui/Card';
import { Modal } from '@/components/ui/Modal';
import { ProfileModal, PasswordModal } from '@/components/modals';
import { useProfileStore } from '@/stores/profileStore';
import { api } from '@/lib/tauri';
import { cn } from '@/lib/cn';
import {DndContext, closestCenter, PointerSensor, useSensor, useSensors, type DragEndEvent} from "@dnd-kit/core";
import {SortableContext, rectSortingStrategy} from "@dnd-kit/sortable";
import SortableCardShell from "../components/ui/SortableCardShell.tsx";


export function Profiles() {
  const { t } = useTranslation();
  const { profiles, reorderProfiles,  current, loading, error, selectProfile, duplicateProfile, deleteProfile, loadProfiles, unlockProfile } = useProfileStore();
  const [editModalOpen, setEditModalOpen] = useState(false);
  const [createModalOpen, setCreateModalOpen] = useState(false);

  // Encryption modal state
  const [encryptModalOpen, setEncryptModalOpen] = useState(false);
  const [encryptingProfileName, setEncryptingProfileName] = useState<string | null>(null);
  const [encryptError, setEncryptError] = useState<string | undefined>();

  // Delete confirmation state
  const [deleteModalOpen, setDeleteModalOpen] = useState(false);
  const [deletingProfileName, setDeletingProfileName] = useState<string | null>(null);
  const [pendingDeleteProfileName, setPendingDeleteProfileName] = useState<string | null>(null);

  // Track which encrypted profiles have been unlocked (password entered) in this session
  const [unlockedProfiles, setUnlockedProfiles] = useState<Set<string>>(new Set());

  // Sensors for DND Kit for Drag and Drop
  const sensors = useSensors( useSensor(PointerSensor, {activationConstraint: {distance: 6 }}));

  // Detect when an encrypted profile is successfully loaded (password was entered)
  useEffect(() => {
    const unsubscribe = useProfileStore.subscribe((state, prevState) => {
      // If current profile changed and new one is loaded
      if (state.current && state.current !== prevState.current) {
        const profileName = state.current.name;
        const profileSummary = state.profiles.find(p => p.name === profileName);

        // If this profile is encrypted, mark it as unlocked in session
        if (profileSummary?.isEncrypted) {
          setUnlockedProfiles(prev => new Set(prev).add(profileName));

          // If this was a pending delete, now show the delete confirmation
          if (pendingDeleteProfileName === profileName) {
            setPendingDeleteProfileName(null);
            setDeletingProfileName(profileName);
            setDeleteModalOpen(true);
          }
        }
      }

      // If encryption was removed (pendingUnlock flow completed), also mark as unlocked
      if (prevState.pendingUnlock && !state.pendingUnlock && state.current) {
        setUnlockedProfiles(prev => new Set(prev).add(state.current!.name));
      }
    });
    return unsubscribe;
  }, [pendingDeleteProfileName]);

  // Clear unlocked state when clicking outside profile cards
  const handleClickAway = (e: React.MouseEvent) => {
    if (e.target === e.currentTarget && unlockedProfiles.size > 0) {
      setUnlockedProfiles(new Set());
    }
  };

  // Clear unlocked state for other profiles when selecting a different one
  const handleProfileClick = (profileName: string) => {
    // Keep only the newly selected profile in unlocked set (if it was unlocked)
    if (unlockedProfiles.has(profileName)) {
      setUnlockedProfiles(new Set([profileName]));
    } else {
      setUnlockedProfiles(new Set());
    }
    selectProfile(profileName);
  };

  // Handle locking a profile with password
  const handleLockProfile = (profileName: string) => {
    // Clear from unlocked set when re-locking
    setUnlockedProfiles(prev => {
      const next = new Set(prev);
      next.delete(profileName);
      return next;
    });
    setEncryptingProfileName(profileName);
    setEncryptError(undefined);
    setEncryptModalOpen(true);
  };

  // Handle password submission for encrypting
  const handleEncryptSubmit = async (password: string) => {
    if (!encryptingProfileName) return;

    setEncryptError(undefined);

    try {
      // Load the profile first (it should already be loaded if user clicked on it)
      const profile = await api.profile.load(encryptingProfileName);
      // Save with password to encrypt it
      await api.profile.save(profile, password);
      // Reload profiles to update the list with encryption status
      await loadProfiles();
      setEncryptModalOpen(false);
      setEncryptingProfileName(null);
    } catch (err) {
      setEncryptError(String(err));
    }
  };

  // Handle removing password protection
  const handleUnlockProfile = (profileName: string) => {
    // Use the store's unlockProfile function which:
    // 1. Sets pendingUnlock flag
    // 2. Triggers password modal
    // 3. After successful password entry, saves without password to remove encryption
    unlockProfile(profileName);
  };

  // Handle delete profile
  const handleDeleteClick = (profileName: string) => {
    const profileSummary = profiles.find(p => p.name === profileName);

    // If profile is encrypted and NOT unlocked in this session, require password first
    if (profileSummary?.isEncrypted && !unlockedProfiles.has(profileName)) {
      // Set pending delete and trigger password modal
      setPendingDeleteProfileName(profileName);
      selectProfile(profileName);
      return;
    }

    // Profile is either not encrypted or already unlocked - show delete confirmation
    setDeletingProfileName(profileName);
    setDeleteModalOpen(true);
  };

  const handleDeleteConfirm = async () => {
    if (deletingProfileName) {
      await deleteProfile(deletingProfileName);
      // Clear from unlocked set after deletion
      setUnlockedProfiles(prev => {
        const next = new Set(prev);
        next.delete(deletingProfileName);
        return next;
      });
      setDeleteModalOpen(false);
      setDeletingProfileName(null);
    }
  };

  const handleDeleteCancel = () => {
    setDeleteModalOpen(false);
    setDeletingProfileName(null);
    setPendingDeleteProfileName(null);
  };

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
        <div className="text-[var(--error-text)]">
          {t('common.error')}: {error}
        </div>
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
                {t('profiles.noProfilesYet')}
              </h3>
              <p
                className="text-[var(--text-secondary)] max-w-md mx-auto"
                style={{ marginBottom: '24px' }}
              >
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

  
  const handleDragEnd = async ({ active, over }: DragEndEvent) => {
    if (!over || active.id === over.id) return;

    const ids = profiles.map((p) => p.name);
    const fromIndex = ids.indexOf(String(active.id));
    const toIndex = ids.indexOf(String(over.id));

    if (fromIndex === -1 || toIndex === -1) return;

    await reorderProfiles(fromIndex, toIndex);
  };


  return (
    <>
      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
        <SortableContext items={profiles.map((p) => p.name)} strategy={rectSortingStrategy}>
          <Grid cols={2} onClick={handleClickAway}>
            {profiles.map((profile) => (
              <SortableCardShell key={profile.name} id={profile.name}>
              <ProfileCard
                key={profile.id}
                name={profile.name}
                meta={[
                  { icon: <Monitor className="w-4 h-4" />, label: profile.resolution },
                  { icon: <Gauge className="w-4 h-4" />, label: `${profile.bitrate} kbps` },
                  {
                    icon: <Target className="w-4 h-4" />,
                    label: t('profiles.targetsCount', { count: profile.targetCount }),
                  },
                ]}
                services={profile.services}
                active={current?.id === profile.id}
                onClick={() => handleProfileClick(profile.name)}
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
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={(e) => {
                        e.stopPropagation();
                        handleDeleteClick(profile.name);
                      }}
                      title={t('common.delete')}
                      className="text-[var(--error-text)] hover:bg-[var(--error-subtle)]"
                    >
                      <Trash2 className="w-4 h-4" />
                    </Button>
                    {(() => {
                      // Check if this encrypted profile has been unlocked in this session
                      const isUnlocked = unlockedProfiles.has(profile.name);
                      // Show Lock icon only if encrypted AND not unlocked in session
                      const showLocked = profile.isEncrypted && !isUnlocked;

                      return showLocked ? (
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={(e) => {
                            e.stopPropagation();
                            selectProfile(profile.name); // This will trigger password modal
                          }}
                          title={t('profiles.enterPassword')}
                        >
                          <Lock className="w-4 h-4 transition-transform duration-300" />
                        </Button>
                      ) : (
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={(e) => {
                            e.stopPropagation();
                            // If unlocked (was encrypted), clicking can remove encryption or re-lock
                            // If not encrypted, clicking adds encryption
                            if (isUnlocked && profile.isEncrypted) {
                              handleUnlockProfile(profile.name); // Remove encryption entirely
                            } else {
                              handleLockProfile(profile.name); // Add encryption
                            }
                          }}
                          title={isUnlocked && profile.isEncrypted
                            ? t('profiles.removePassword')
                            : t('profiles.addPassword')}
                        >
                          <Unlock
                            className={cn(
                              "w-4 h-4 transition-all duration-300",
                              isUnlocked && "text-[var(--success)] scale-110"
                            )}
                          />
                        </Button>
                      );
                    })()}
                  </div>
                }
              />
              </SortableCardShell>
            ))}

            {/* Add New Profile Card */}
            <Card
              className="border-2 border-dashed border-[var(--border-default)] hover:border-[var(--primary)] transition-colors cursor-pointer"
              onClick={() => setCreateModalOpen(true)}
            >
              <CardBody
                className="flex flex-col items-center justify-center"
                style={{ padding: '56px 28px' }}
              >
                <div
                  className="w-14 h-14 rounded-full bg-[var(--primary-subtle)] flex items-center justify-center"
                  style={{ marginBottom: '16px' }}
                >
                  <Plus className="w-7 h-7 text-[var(--primary)]" />
                </div>
                <span className="text-base font-medium text-[var(--text-secondary)]">
                  {t('profiles.createNewProfile')}
                </span>
              </CardBody>
            </Card>
          </Grid>
        </SortableContext>
      </DndContext>

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

      {/* Encrypt Profile Modal */}
      <PasswordModal
        open={encryptModalOpen}
        onClose={() => {
          setEncryptModalOpen(false);
          setEncryptingProfileName(null);
          setEncryptError(undefined);
        }}
        onSubmit={handleEncryptSubmit}
        mode="encrypt"
        profileName={encryptingProfileName || undefined}
        error={encryptError}
      />

      {/* Delete Confirmation Modal */}
      <Modal
        open={deleteModalOpen}
        onClose={handleDeleteCancel}
        title={t('profiles.deleteProfile')}
        footer={
          <>
            <Button variant="ghost" onClick={handleDeleteCancel}>
              {t('common.cancel')}
            </Button>
            <Button variant="destructive" onClick={handleDeleteConfirm}>
              {t('common.delete')}
            </Button>
          </>
        }
      >
        <p className="text-[var(--text-secondary)]">
          {t('profiles.deleteConfirmation', { name: deletingProfileName })}
        </p>
      </Modal>
    </>
  );
}
