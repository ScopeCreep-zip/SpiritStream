import React, { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Modal } from '@/components/ui/Modal';
import { SettingsRail, type RailItem } from './SettingsRail';
import type { SettingsSection } from '@/hooks/useModalRegistry';
import { ObsPanel } from '@/components/integrations/ObsPanel';
import { ChatPanel } from '@/components/integrations/ChatPanel';
import { DiscordPanel } from '@/components/integrations/DiscordPanel';
import { Settings } from '@/views/Settings';
import { LogsViewer } from '@/components/logs/LogsViewer';
import { AuditLogViewer } from '@/components/audit/AuditLogViewer';
import { SafetyWizard, type SafetyWizardResult } from '@/views/SafetyWizard';
import { ShortcutsList } from '@/components/help/ShortcutsList';
import { ProfileForm } from '@/components/forms/ProfileForm';
import { OutputGroupForm } from '@/components/forms/OutputGroupForm';
import type { ChatPlatform, Profile, OutputGroup } from '@spiritstream/types';

export interface SettingsWindowProps {
  open: boolean;
  section: SettingsSection | null;
  onSectionChange: (s: SettingsSection) => void;
  onClose: () => void;
  /** Persist the safety-wizard choices (owned by the shell). */
  onSafetyComplete: (result: SafetyWizardResult) => void;
  /** Active profile for the "Edit profile" section. */
  editProfile?: Profile;
  /** Active output group for the "Encoder" section. The section is shown
   *  ONLY when this is a custom (non-default) group — the passthrough group
   *  has no encoder settings. */
  editGroup?: OutputGroup;
  /** Deep-link the Chat section to a specific platform (scroll to its card),
   *  e.g. from a stream-target row's chat icon. */
  chatInitialPlatform?: ChatPlatform;
}

interface SectionDef {
  readonly id: SettingsSection;
  readonly label: string;
  readonly render: () => React.ReactElement;
}

/**
 * Unified settings window: ONE dismissible window whose left tab-rail switches
 * between every settings/inspection destination (OBS, Chat, Discord, App
 * settings, Logs, Audit log, Safety wizard, Shortcuts, Edit profile). Reuses
 * the existing `Modal` shell for overlay + focus-trap + Escape; each section is
 * an existing self-contained panel, mounted only while active. The flat rail
 * order is the spec's. The Modal title is generic ("Settings") because each
 * panel renders its own heading.
 */
export function SettingsWindow({
  open,
  section,
  onSectionChange,
  onClose,
  onSafetyComplete,
  editProfile,
  editGroup,
  chatInitialPlatform,
}: SettingsWindowProps): React.ReactElement | null {
  const { t } = useTranslation();

  // Human reading order: YOU → YOUR SERVICES → THE APP.
  //   1. user/profile  — Edit profile, Safety wizard (configures your own
  //      protection: profile, PII blocklist, anonymous mode)
  //   2. services      — OBS, Chat, Discord, Encoder (stream in/out config)
  //   3. application   — App settings, Logs, Audit log, Shortcuts
  // The Encoder section only appears when a custom (non-default) output group
  // is active — the passthrough group has no encoder settings.
  const canEditEncoder = editGroup != null && !editGroup.isDefault;
  const sections: ReadonlyArray<SectionDef> = useMemo(() => {
    const list: SectionDef[] = [
      {
        id: 'profileEdit',
        label: t('settings.rail.profileEdit', { defaultValue: 'Edit profile' }),
        render: () => (
          <ProfileForm mode="edit" profile={editProfile} onDone={onClose} onCancel={onClose} />
        ),
      },
      {
        id: 'safetyWizard',
        label: t('settings.rail.safetyWizard', { defaultValue: 'Safety wizard' }),
        render: () => <SafetyWizard onComplete={onSafetyComplete} />,
      },
      { id: 'obs', label: t('settings.rail.obs', { defaultValue: 'OBS' }), render: () => <ObsPanel /> },
      {
        id: 'chat',
        label: t('settings.rail.chat', { defaultValue: 'Chat' }),
        render: () => <ChatPanel initialPlatform={chatInitialPlatform} />,
      },
      {
        id: 'discord',
        label: t('settings.rail.discord', { defaultValue: 'Discord' }),
        render: () => <DiscordPanel />,
      },
    ];
    if (canEditEncoder && editGroup) {
      list.push({
        id: 'encoder',
        label: t('settings.rail.encoder', { defaultValue: 'Encoder' }),
        render: () => (
          <OutputGroupForm mode="edit" group={editGroup} onDone={onClose} onCancel={onClose} />
        ),
      });
    }
    list.push(
      {
        id: 'settings',
        label: t('settings.rail.appSettings', { defaultValue: 'App settings' }),
        render: () => <Settings />,
      },
      { id: 'logs', label: t('settings.rail.logs', { defaultValue: 'Logs' }), render: () => <LogsViewer /> },
      {
        id: 'audit',
        label: t('settings.rail.audit', { defaultValue: 'Audit log' }),
        render: () => <AuditLogViewer />,
      },
      {
        id: 'shortcuts',
        label: t('settings.rail.shortcuts', { defaultValue: 'Shortcuts' }),
        render: () => <ShortcutsList />,
      }
    );
    return list;
  }, [t, onSafetyComplete, onClose, editProfile, editGroup, canEditEncoder, chatInitialPlatform]);

  if (!open || section == null) return null;

  const items: ReadonlyArray<RailItem> = sections.map((s) => ({ id: s.id, label: s.label }));
  const active = sections.find((s) => s.id === section) ?? sections[0];
  const windowTitle = t('settings.windowTitle', { defaultValue: 'Settings' });

  return (
    <Modal open={open} onClose={onClose} title={windowTitle} maxWidth="1040px" closeOnBackdropClick>
      {/* Fixed height so the rail and panel each scroll INDEPENDENTLY — without
          it the whole modal body scrolls, dragging the rail out of view when a
          deep-link scrolls the panel. */}
      <div className="flex gap-4 h-[70vh]">
        <SettingsRail items={items} active={active.id} onSelect={onSectionChange} ariaLabel={windowTitle} />
        <div
          role="tabpanel"
          id={`settings-panel-${active.id}`}
          aria-labelledby={`settings-tab-${active.id}`}
          tabIndex={0}
          className="flex-1 min-w-0 overflow-y-auto focus-visible:outline-none"
        >
          {active.render()}
        </div>
      </div>
    </Modal>
  );
}
