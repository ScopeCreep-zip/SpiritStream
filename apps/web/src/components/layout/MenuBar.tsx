import React from 'react';
import * as Menubar from '@radix-ui/react-menubar';
import { useTranslation } from 'react-i18next';
import { FileMenu } from '@/components/menu/FileMenu';
import { ProfileMenu } from '@/components/menu/ProfileMenu';
import { StreamMenu } from '@/components/menu/StreamMenu';
import { ToolsMenu } from '@/components/menu/ToolsMenu';
import { SafetyMenu } from '@/components/menu/SafetyMenu';
import { ViewMenu } from '@/components/menu/ViewMenu';
import { HelpMenu } from '@/components/menu/HelpMenu';
import type { ModalName } from '@/hooks/useModalRegistry';

interface MenuBarProps {
  onOpenModal: (name: ModalName) => void;
  /** Toggle the chat column collapsed/expanded state. */
  onToggleChat: () => void;
  chatCollapsed: boolean;
  /** Stream → Encoder Settings dispatches here so the shell can target the active group. */
  onEditEncoder: () => void;
  /** Whether an encoder is available to edit (i.e. an output group exists on the active profile). */
  canEditEncoder: boolean;
}

/**
 * Top menu bar. Pure composition — each menu lives in its own module
 * under components/menu/ and owns its store subscriptions + local
 * handlers. The visible menu structure is byte-identical to the
 * pre-split version: same items, same order, same submenus, same
 * keyboard shortcuts, same dispatches.
 */
export function MenuBar({
  onOpenModal,
  onToggleChat,
  chatCollapsed,
  onEditEncoder,
  canEditEncoder,
}: MenuBarProps): React.ReactElement {
  const { t } = useTranslation();

  return (
    <Menubar.Root
      className="flex items-center gap-1 px-3 h-[var(--menubar-h,40px)] bg-bg-surface border-b border-border-default"
      aria-label={t('menu.bar', { defaultValue: 'Application menu' })}
    >
      <span className="ps-1 pe-3 text-sm font-semibold text-text-primary">SpiritStream</span>
      <FileMenu onOpenModal={onOpenModal} />
      <ProfileMenu onOpenModal={onOpenModal} />
      <StreamMenu onEditEncoder={onEditEncoder} canEditEncoder={canEditEncoder} />
      <ToolsMenu onOpenModal={onOpenModal} />
      <SafetyMenu onOpenModal={onOpenModal} />
      <ViewMenu onToggleChat={onToggleChat} chatCollapsed={chatCollapsed} />
      <HelpMenu onOpenModal={onOpenModal} />
    </Menubar.Root>
  );
}
