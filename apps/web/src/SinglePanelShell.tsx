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
import { ShortcutsOverlay } from '@/components/help/ShortcutsOverlay';
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
import { SafetyWizard } from '@/views/SafetyWizard';
import { LogsViewer } from '@/components/logs/LogsViewer';
import { AuditLogViewer } from '@/components/audit/AuditLogViewer';
import { Settings } from '@/views/Settings';
import { ObsPanel } from '@/components/integrations/ObsPanel';
import { DiscordPanel } from '@/components/integrations/DiscordPanel';
import type { Platform, OutputGroup, StreamTarget } from '@spiritstream/types';

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
  } = useProfileStore();

  const { state: modals, open, close } = useModalRegistry();
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
    [open],
  );

  /** AppDrawer → user picked a service. Hand off to targetCreate modal. */
  const handleDrawerSelect = useCallback(
    (platform: Platform) => {
      setPendingService(platform);
      open('targetCreate');
    },
    [open],
  );

  /** Pipeline → row-level edit. Stash the (group, target) pair and open targetEdit. */
  const handleEditTarget = useCallback(
    (group: OutputGroup, target: StreamTarget) => {
      setEditingTarget({ group, target });
      open('targetEdit');
    },
    [open],
  );

  /** Pipeline + Input encoder card → open OutputGroupModal in edit mode. */
  const handleEditGroup = useCallback(
    (group: OutputGroup) => {
      setEditingGroup(group);
      open('outputGroupEdit');
    },
    [open],
  );

  /** Stream menu → Encoder Settings on the active group. */
  const handleEditActiveEncoder = useCallback(() => {
    if (activeGroup) handleEditGroup(activeGroup);
  }, [activeGroup, handleEditGroup]);

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
        onToggleChat={toggleChat}
        chatCollapsed={chatCollapsed}
        onEditEncoder={handleEditActiveEncoder}
        canEditEncoder={!!activeGroup}
      />

      <StatusStrip profile={current} onOpenModal={open} />

      <Canvas
        chatCollapsed={chatCollapsed}
        onToggleChat={toggleChat}
        input={
          <InputColumn
            profile={current}
            activeGroup={activeGroup}
            onConfigureSource={() => open('profileEdit')}
            onEditEncoder={handleEditGroup}
            onConfigureObs={() => open('obs')}
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
          />
        }
        chat={<Chat />}
      />

      {/* ─── Profile modals ─── */}
      <ProfileModal open={modals.profileCreate} onClose={() => close('profileCreate')} mode="create" />
      <ProfileModal
        open={modals.profileEdit}
        onClose={() => close('profileEdit')}
        mode="edit"
        profile={current ?? undefined}
      />
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

      {/* ─── Tools menu ─── */}
      <ViewModal
        open={modals.obs}
        onClose={() => close('obs')}
        title={t('menu.tools.obs', { defaultValue: 'OBS connection' })}
        maxWidth="720px"
      >
        <ObsPanel />
      </ViewModal>
      <ViewModal
        open={modals.discord}
        onClose={() => close('discord')}
        title={t('menu.tools.discord', { defaultValue: 'Discord notifications' })}
        maxWidth="720px"
      >
        <DiscordPanel />
      </ViewModal>
      <ViewModal
        open={modals.settings}
        onClose={() => close('settings')}
        title={t('menu.tools.settings', { defaultValue: 'Settings' })}
        maxWidth="960px"
      >
        <Settings />
      </ViewModal>
      <ViewModal
        open={modals.audit}
        onClose={() => close('audit')}
        title={t('menu.tools.audit', { defaultValue: 'Audit log' })}
        maxWidth="900px"
      >
        <AuditLogViewer />
      </ViewModal>
      <ViewModal
        open={modals.logs}
        onClose={() => close('logs')}
        title={t('menu.tools.logs', { defaultValue: 'Logs' })}
        maxWidth="900px"
      >
        <LogsViewer />
      </ViewModal>

      {/* ─── Safety / Help ─── */}
      {modals.safetyWizard && (
        <Modal
          open
          onClose={() => close('safetyWizard')}
          title={t('safety.wizard.title', { defaultValue: 'Safety wizard' })}
          maxWidth="640px"
        >
          <SafetyWizard
            onComplete={() => close('safetyWizard')}
            onSkip={() => close('safetyWizard')}
          />
        </Modal>
      )}
      <ShortcutsOverlay open={modals.shortcuts} onClose={() => close('shortcuts')} />

      {/* ─── Always-on auth + session leaves ─── */}
      <PasswordModal
        open={!!pendingPasswordProfile}
        onClose={cancelPasswordPrompt}
        onSubmit={submitPassword}
        mode="decrypt"
        profileName={pendingPasswordProfile ?? undefined}
        error={passwordError ?? undefined}
      />
      <LoginModal open={loginModalOpen} onSuccess={onLoginSuccess} />

      <ToastContainer />
    </div>
  );
}

/**
 * Mount an existing view as the body of a generic modal. Kept around for
 * Settings / Logs / Audit / OBS / Discord — these views are stable read /
 * config surfaces that don't fit the column shape and don't block the
 * single-panel UX.
 */
interface ViewModalProps {
  open: boolean;
  onClose: () => void;
  title: string;
  maxWidth?: string;
  children: React.ReactNode;
}

function ViewModal({ open, onClose, title, maxWidth, children }: ViewModalProps): React.ReactElement {
  return (
    <Modal open={open} onClose={onClose} title={title} maxWidth={maxWidth ?? '800px'} closeOnBackdropClick>
      {children}
    </Modal>
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
          {profiles.map((p) => (
            <li key={p.name}>
              <button
                type="button"
                onClick={async () => {
                  await selectProfile(p.name);
                  onClose();
                }}
                className="w-full text-start px-3 py-2 rounded-md text-text-primary hover:bg-bg-hover focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default"
                aria-current={current?.name === p.name ? 'true' : undefined}
              >
                {p.name}
                {current?.name === p.name && (
                  <span className="ms-2 text-text-tertiary text-xs">
                    ({t('common.current', { defaultValue: 'current' })})
                  </span>
                )}
              </button>
            </li>
          ))}
        </ul>
      )}
    </Modal>
  );
}
