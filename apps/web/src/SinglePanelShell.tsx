/**
 * The single-panel app shell.
 *
 * Layout:
 *   - MenuBar (Radix menubar: File / Profile / Stream / Tools / Safety / View / Help)
 *   - StatusStrip (connection dot, profile pill, LIVE badge, START/STOP, PANIC 48×48)
 *   - Canvas (three-column grid: input | pipeline | collapsible chat)
 *
 * Inline edits flow through the existing TargetModal / OutputGroupModal in
 * edit mode — no per-view "open the whole view as a modal body" wrappers.
 * The AppDrawer (searchable Platform catalog) handles the add-target flow.
 *
 * A handful of stable read/config surfaces (Settings, Logs, AuditLog,
 * ObsPanel, DiscordPanel) are still mounted as `<ViewModal>` bodies —
 * they don't fit the column shape and don't block the single-panel UX.
 *
 * All business logic stays in `crates/core`. Every action here either
 * dispatches a Zustand store action or calls an existing `api.*` method.
 */

import React, { useCallback, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useProfileStore } from '@/stores/profileStore';
import { useModalRegistry } from '@/hooks/useModalRegistry';
import { useDocumentTitle } from '@/hooks/useDocumentTitle';
import { usePanicHotkey } from '@/hooks/usePanicHotkey';
import { MenuBar } from '@/components/layout/MenuBar';
import { StatusStrip } from '@/components/layout/StatusStrip';
import { Canvas } from '@/components/layout/Canvas';
import { ProfileModal } from '@/components/modals/ProfileModal';
import { TargetModal } from '@/components/modals/TargetModal';
import { OutputGroupModal } from '@/components/modals/OutputGroupModal';
import { PasswordModal } from '@/components/modals/PasswordModal';
import { LoginModal } from '@/components/modals/LoginModal';
import { Modal } from '@/components/ui/Modal';
import { ToastContainer } from '@/components/ui/Toast';
import { InputColumn } from '@/components/input/InputColumn';
import { PipelineColumn } from '@/components/pipeline/PipelineColumn';
import { AppDrawer } from '@/components/drawer/AppDrawer';
import { Chat } from '@/views/Chat';
import { SettingsWindow } from '@/components/settings/SettingsWindow';
import type { SafetyWizardResult } from '@/views/SafetyWizard';
import { ChevronUp, ChevronDown, Copy } from 'lucide-react';
import { toast } from '@/hooks/useToast';
import { logger } from '@/lib/logger';
import type { ChatPlatform, Platform, OutputGroup, StreamTarget } from '@spiritstream/types';

export interface SinglePanelShellProps {
  /** Auth gate state owned by AppContent — surfaced here so the LoginModal renders alongside the shell. */
  loginModalOpen: boolean;
  onLoginSuccess: () => void;
}

interface EditingTarget {
  group: OutputGroup;
  target: StreamTarget;
}

export function SinglePanelShell({
  loginModalOpen,
  onLoginSuccess,
}: SinglePanelShellProps): React.ReactElement {
  const { t } = useTranslation();
  const {
    current,
    pendingPasswordProfile,
    submitPassword,
    cancelPasswordPrompt,
    passwordError,
    clearPasswordError,
    updateProfile,
  } = useProfileStore();

  const { state: modals, open, close, settingsSection, openSettings, closeSettings } =
    useModalRegistry();

  /**
   * Persist the wizard's choices onto the active profile. The wizard is
   * pure presentation; the blocklist normalization (trim/dedupe) and
   * the follower-only application both happen backend-side. On failure we
   * surface an error toast and keep the choices on screen — a vulnerable
   * user must never believe their blocklist is armed when the save failed.
   */
  const handleSafetyWizardComplete = useCallback(
    async (result: SafetyWizardResult): Promise<void> => {
      const profile = useProfileStore.getState().current;
      if (!profile) {
        toast.error(
          t('safety.wizard.noProfile', {
            defaultValue: 'Open or create a profile before running the safety wizard.',
          })
        );
        return;
      }
      try {
        await updateProfile({
          piiBlocklist: result.piiBlocklist,
          anonymousLogging: result.anonymousLogging,
          settings: {
            ...profile.settings,
            chat: {
              ...profile.settings.chat,
              followerOnlyDefault: result.followerOnlyDefault,
            },
          },
        });
        toast.success(
          t('safety.wizard.savedToast', {
            defaultValue: 'Safety settings saved to your profile.',
          })
        );
        // Stay in the settings window — finishing the wizard saves in place
        // (the success toast is the confirmation); it must NOT close the
        // whole window the way the old standalone first-run modal did.
      } catch (error) {
        logger.error('[SinglePanelShell] safety wizard save failed:', error);
        toast.error(
          t('safety.wizard.saveFailedToast', {
            defaultValue: 'Saving safety settings failed — your choices were NOT applied.',
          })
        );
      }
    },
    [updateProfile, t]
  );
  const [chatCollapsed, setChatCollapsed] = useState(false);
  /**
   * User's chosen output group. May be stale (e.g., after profile switch),
   * so `activeGroup` below falls back to the first group when the id doesn't
   * resolve. Cross-column state (Input reads, Pipeline controls) lives here.
   */
  const [selectedGroupId, setSelectedGroupId] = useState<string | null>(null);

  /**
   * Editing handoff state. PipelineColumn / InputColumn raise these via
   * specialised callbacks; the shell reads them when mounting the
   * TargetModal / OutputGroupModal in edit mode.
   */
  const [editingTarget, setEditingTarget] = useState<EditingTarget | null>(null);
  const [editingGroup, setEditingGroup] = useState<OutputGroup | null>(null);

  /**
   * AppDrawer → TargetModal handoff. When pipeline opens the drawer, we
   * remember which group the user is adding to. After selection, the
   * platform pre-fills the targetCreate modal via `initialService`.
   */
  const [drawerGroupId, setDrawerGroupId] = useState<string | null>(null);
  const [pendingService, setPendingService] = useState<Platform | null>(null);

  /**
   * When a target row's chat icon is clicked, the unified settings window opens
   * to the Chat section deep-linked to that platform (ChatPanel scrolls to its
   * card). Cleared on window close so a later plain "Tools → Chat" doesn't
   * re-scroll to a stale platform.
   */
  const [chatInitialPlatform, setChatInitialPlatform] = useState<ChatPlatform | undefined>(
    undefined
  );

  useDocumentTitle();
  usePanicHotkey();

  const toggleChat = useCallback(() => setChatCollapsed((c) => !c), []);

  const activeGroup = useMemo(() => {
    const groups = current?.outputGroups ?? [];
    return groups.find((g) => g.id === selectedGroupId) ?? groups[0] ?? null;
  }, [current, selectedGroupId]);

  /** Pipeline → Add Target (open AppDrawer pinned to a specific group). */
  const handleAddTargetForGroup = useCallback(
    (group: OutputGroup) => {
      setDrawerGroupId(group.id);
      open('appDrawerStream');
    },
    [open]
  );

  /** AppDrawer → user picked a service. Hand off to targetCreate modal. */
  const handleDrawerSelect = useCallback(
    (platform: Platform) => {
      setPendingService(platform);
      open('targetCreate');
    },
    [open]
  );

  /** Pipeline → row-level edit. Stash the (group, target) pair and open targetEdit. */
  const handleEditTarget = useCallback(
    (group: OutputGroup, target: StreamTarget) => {
      setEditingTarget({ group, target });
      open('targetEdit');
    },
    [open]
  );

  /** Pipeline + Input encoder card → open OutputGroupModal in edit mode. */
  const handleEditGroup = useCallback(
    (group: OutputGroup) => {
      setEditingGroup(group);
      open('outputGroupEdit');
    },
    [open]
  );

  /**
   * "Encoder settings" on the ACTIVE group → opens the unified settings
   * window's Encoder section (Stream menu + Input encoder card). Only the
   * passthrough group has no encoder settings, so it's a no-op there. Editing
   * a SPECIFIC group from its pipeline row stays the focused OutputGroupModal
   * (`handleEditGroup`).
   */
  const openEncoderSettings = useCallback(() => {
    if (activeGroup && !activeGroup.isDefault) openSettings('encoder');
  }, [activeGroup, openSettings]);

  /** Row chat icon → open the settings window's Chat section, scrolled to the
   *  target's platform. */
  const handleOpenChatSettings = useCallback(
    (platform: ChatPlatform) => {
      setChatInitialPlatform(platform);
      openSettings('chat');
    },
    [openSettings]
  );

  /** Close the settings window and drop any chat deep-link. */
  const handleCloseSettings = useCallback(() => {
    closeSettings();
    setChatInitialPlatform(undefined);
  }, [closeSettings]);

  return (
    <div className="flex flex-col h-screen bg-bg-base text-text-primary">
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:absolute focus:top-2 focus:left-2 focus:z-50 focus:px-3 focus:py-2 focus:bg-bg-elevated focus:text-text-primary focus:rounded-md focus:ring-[3px] focus:ring-ring-default"
      >
        {t('a11y.skipToMain', { defaultValue: 'Skip to main content' })}
      </a>

      <MenuBar
        onOpenModal={open}
        onOpenSettings={openSettings}
        onToggleChat={toggleChat}
        chatCollapsed={chatCollapsed}
        onEditEncoder={openEncoderSettings}
        canEditEncoder={!!activeGroup && !activeGroup.isDefault}
      />

      <StatusStrip profile={current} onOpenModal={open} />

      <Canvas
        chatCollapsed={chatCollapsed}
        onToggleChat={toggleChat}
        input={
          <InputColumn
            profile={current}
            activeGroup={activeGroup}
            onConfigureSource={() => openSettings('profileEdit')}
            onEditEncoder={openEncoderSettings}
            onConfigureObs={() => openSettings('obs')}
          />
        }
        pipeline={
          <PipelineColumn
            profile={current}
            activeGroupId={activeGroup?.id ?? null}
            onSelectGroup={setSelectedGroupId}
            onAddTargetForGroup={handleAddTargetForGroup}
            onEditTarget={handleEditTarget}
            onEditGroup={handleEditGroup}
            onOpenChatSettings={handleOpenChatSettings}
          />
        }
        chat={<Chat onOpenIntegrations={() => openSettings('chat')} />}
      />

      {/* ─── Profile modals ─── */}
      <ProfileModal open={modals.profileCreate} onClose={() => close('profileCreate')} />
      {modals.openProfile && <OpenProfileModal open onClose={() => close('openProfile')} />}

      {/* ─── Target modals (create from AppDrawer / edit from row) ─── */}
      <TargetModal
        open={modals.targetCreate}
        onClose={() => {
          close('targetCreate');
          setPendingService(null);
          setDrawerGroupId(null);
        }}
        mode="create"
        groupId={drawerGroupId ?? activeGroup?.id ?? ''}
        initialService={pendingService ?? undefined}
      />
      <TargetModal
        open={modals.targetEdit}
        onClose={() => {
          close('targetEdit');
          setEditingTarget(null);
        }}
        mode="edit"
        groupId={editingTarget?.group.id ?? ''}
        target={editingTarget?.target}
      />

      {/* ─── Output-group modals (create from pipeline / edit from row / encoder) ─── */}
      <OutputGroupModal
        open={modals.outputGroupCreate}
        onClose={() => close('outputGroupCreate')}
        mode="create"
      />
      <OutputGroupModal
        open={modals.outputGroupEdit}
        onClose={() => {
          close('outputGroupEdit');
          setEditingGroup(null);
        }}
        mode="edit"
        group={editingGroup ?? undefined}
      />

      {/* ─── AppDrawer (service catalog) ─── */}
      <AppDrawer
        open={modals.appDrawerStream}
        onClose={() => close('appDrawerStream')}
        mode="stream-target"
        onSelect={handleDrawerSelect}
      />

      {/* ─── Unified settings window (Tools / Safety / Help / Profile destinations) ─── */}
      <SettingsWindow
        open={settingsSection != null}
        section={settingsSection}
        onSectionChange={openSettings}
        onClose={handleCloseSettings}
        onSafetyComplete={handleSafetyWizardComplete}
        editProfile={current ?? undefined}
        editGroup={activeGroup ?? undefined}
        chatInitialPlatform={chatInitialPlatform}
      />

      {/* ─── Always-on auth + session leaves ─── */}
      <PasswordModal
        open={!!pendingPasswordProfile}
        onClose={cancelPasswordPrompt}
        onSubmit={submitPassword}
        mode="decrypt"
        profileName={pendingPasswordProfile ?? undefined}
        error={passwordError ?? undefined}
        onErrorClear={clearPasswordError}
      />
      <LoginModal open={loginModalOpen} onSuccess={onLoginSuccess} />

      <ToastContainer />
    </div>
  );
}

interface OpenProfileModalProps {
  open: boolean;
  onClose: () => void;
}

function OpenProfileModal({ open, onClose }: OpenProfileModalProps): React.ReactElement {
  const { t } = useTranslation();
  const profiles = useProfileStore((s) => s.profiles);
  const current = useProfileStore((s) => s.current);
  const selectProfile = useProfileStore((s) => s.selectProfile);
  const reorderProfiles = useProfileStore((s) => s.reorderProfiles);
  const duplicateProfile = useProfileStore((s) => s.duplicateProfile);

  const handleDuplicate = async (name: string): Promise<void> => {
    try {
      await duplicateProfile(name);
      toast.success(t('toast.profileDuplicated', { name, defaultValue: 'Duplicated {{name}}' }));
    } catch (err) {
      toast.error(
        t('toast.profileDuplicateFailed', {
          defaultValue: 'Failed to duplicate {{name}}: {{error}}',
          name,
          error: err instanceof Error ? err.message : String(err),
        })
      );
    }
  };

  const iconButtonClass =
    'p-2 rounded-md text-text-tertiary hover:text-text-primary hover:bg-bg-hover disabled:opacity-40 disabled:pointer-events-none focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default';

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={t('menu.file.openProfile', { defaultValue: 'Open profile' })}
      maxWidth="420px"
      closeOnBackdropClick
    >
      {profiles.length === 0 ? (
        <p className="text-text-secondary text-sm">
          {t('menu.file.noProfiles', { defaultValue: 'No profiles yet — use File → New profile.' })}
        </p>
      ) : (
        <ul className="flex flex-col gap-1">
          {profiles.map((p, index) => (
            <li key={p.name} className="flex items-center gap-1">
              <button
                type="button"
                onClick={async () => {
                  await selectProfile(p.name);
                  onClose();
                }}
                className="flex-1 text-start px-3 py-2 rounded-md text-text-primary hover:bg-bg-hover focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default"
                aria-current={current?.name === p.name ? 'true' : undefined}
              >
                {p.name}
                {current?.name === p.name && (
                  <span className="ms-2 text-text-tertiary text-xs">
                    ({t('common.current', { defaultValue: 'current' })})
                  </span>
                )}
              </button>
              <button
                type="button"
                onClick={() => reorderProfiles(index, index - 1)}
                disabled={index === 0}
                aria-label={t('menu.file.moveProfileUp', {
                  name: p.name,
                  defaultValue: 'Move {{name}} up',
                })}
                className={iconButtonClass}
              >
                <ChevronUp className="w-4 h-4" aria-hidden="true" />
              </button>
              <button
                type="button"
                onClick={() => reorderProfiles(index, index + 1)}
                disabled={index === profiles.length - 1}
                aria-label={t('menu.file.moveProfileDown', {
                  name: p.name,
                  defaultValue: 'Move {{name}} down',
                })}
                className={iconButtonClass}
              >
                <ChevronDown className="w-4 h-4" aria-hidden="true" />
              </button>
              <button
                type="button"
                onClick={() => handleDuplicate(p.name)}
                aria-label={t('menu.file.duplicateProfile', {
                  name: p.name,
                  defaultValue: 'Duplicate {{name}}',
                })}
                className={iconButtonClass}
              >
                <Copy className="w-4 h-4" aria-hidden="true" />
              </button>
            </li>
          ))}
        </ul>
      )}
    </Modal>
  );
}
