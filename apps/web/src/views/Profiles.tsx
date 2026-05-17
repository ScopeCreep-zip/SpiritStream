import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Monitor, Gauge, Target, Lock, Unlock, Trash2 } from 'lucide-react';
import { Grid } from '@/components/ui/Grid';
import { ProfileCard } from '@/components/dashboard/ProfileCard';
import { Button } from '@/components/ui/Button';
import { Card, CardBody } from '@/components/ui/Card';
import { ConfirmDialog } from '@spiritstream/ui';
import { ProfileModal, PasswordModal } from '@/components/modals';
import { useProfileStore } from '@/stores/profileStore';
import { api } from '@/lib/client';
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

  // Mirror of the backend's session unlock set
  // (`POST /api/v1/profiles/{name}/decrypt`, `/lock`, `GET /locked`).
  // The server is authoritative; this local set is a cache for synchronous
  // UI checks. Plan: "unlockedProfiles Set in component state →
  // server-side session state tied to the auth cookie."
  const [unlockedProfiles, setUnlockedProfiles] = useState<Set<string>>(new Set());

  const refreshUnlocked = async () => {
    try {
      const { unlocked } = await api.profile.lockedList();
      setUnlockedProfiles(new Set(unlocked));
    } catch {
      // Non-fatal: fall back to existing cache.
    }
  };

  useEffect(() => { void refreshUnlocked(); }, []);

  // Sensors for DND Kit for Drag and Drop
  const sensors = useSensors( useSensor(PointerSensor, {activationConstraint: {distance: 6 }}));

  // Detect when an encrypted profile is successfully loaded (password was
  // entered): notify the backend so the session unlock set stays canonical.
  useEffect(() => {
    const unsubscribe = useProfileStore.subscribe((state, prevState) => {
      if (state.current && state.current !== prevState.current) {
        const profileName = state.current.name;
        const profileSummary = state.profiles.find(p => p.name === profileName);
        if (profileSummary?.isEncrypted) {
          // The successful load implies a correct password reached the backend.
          // Optimistically mirror locally; refresh the canonical list to confirm.
          setUnlockedProfiles(prev => new Set(prev).add(profileName));
          void refreshUnlocked();
          if (pendingDeleteProfileName === profileName) {
            setPendingDeleteProfileName(null);
            setDeletingProfileName(profileName);
            setDeleteModalOpen(true);
          }
        }
      }
      if (prevState.pendingUnlock && !state.pendingUnlock && state.current) {
        setUnlockedProfiles(prev => new Set(prev).add(state.current!.name));
        void refreshUnlocked();
      }
    });
    return unsubscribe;
  }, [pendingDeleteProfileName]);

  const handleClickAway = async (e: React.MouseEvent) => {
    if (e.target === e.currentTarget && unlockedProfiles.size > 0) {
      const names = Array.from(unlockedProfiles);
      setUnlockedProfiles(new Set());
      await Promise.all(names.map((name) => api.profile.lock(name).catch(() => undefined)));
    }
  };

  const handleProfileClick = async (profileName: string) => {
    if (unlockedProfiles.has(profileName)) {
      const toLock = Array.from(unlockedProfiles).filter((n) => n !== profileName);
      setUnlockedProfiles(new Set([profileName]));
      await Promise.all(toLock.map((name) => api.profile.lock(name).catch(() => undefined)));
    } else {
      const toLock = Array.from(unlockedProfiles);
      setUnlockedProfiles(new Set());
      await Promise.all(toLock.map((name) => api.profile.lock(name).catch(() => undefined)));
    }
    selectProfile(profileName);
  };

  const handleLockProfile = async (profileName: string) => {
    setUnlockedProfiles(prev => {
      const next = new Set(prev);
      next.delete(profileName);
      return next;
    });
    await api.profile.lock(profileName).catch(() => undefined);
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
      const profile = await api.profile.load(encryptingProfileName, undefined, false);
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
        <div className="text-text-secondary">{t('profiles.loadingProfiles')}</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-error-text">
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
            <div className="text-center py-12">
              <div
                className="w-16 h-16 mx-auto rounded-full bg-primary-subtle flex items-center justify-center mb-4"
              >
                <Plus className="w-8 h-8 text-primary" />
              </div>
              <h3
                className="text-lg font-semibold text-text-primary mb-2"
              >
                {t('profiles.noProfilesYet')}
              </h3>
              <p
                className="text-text-secondary max-w-md mx-auto mb-6"
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
                    <div className="flex gap-1">
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
                        className="text-error-text hover:bg-error-subtle"
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
                                isUnlocked && "text-success scale-110"
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
              className="border-2 border-dashed border-border-default hover:border-primary transition-colors cursor-pointer h-full"
              onClick={() => setCreateModalOpen(true)}
            >
              <CardBody
                className="flex flex-col items-center justify-center h-full p-5"
              >
                <div
                  className="w-14 h-14 rounded-full bg-primary-subtle flex items-center justify-center mb-4"
                >
                  <Plus className="w-7 h-7 text-primary" />
                </div>
                <span className="text-base font-medium text-text-secondary">
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

      {/* Delete Profile confirmation. */}
      <ConfirmDialog
        open={deleteModalOpen}
        title={t('profiles.deleteProfile')}
        message={t('profiles.deleteConfirmation', { name: deletingProfileName })}
        confirmLabel={t('common.delete')}
        cancelLabel={t('common.cancel')}
        onConfirm={handleDeleteConfirm}
        onCancel={handleDeleteCancel}
      />
    </>
  );
}
