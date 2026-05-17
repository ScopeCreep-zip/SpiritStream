import React, { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import * as Menubar from '@radix-ui/react-menubar';
import { Check, ChevronRight } from 'lucide-react';
import { api } from '@/lib/client';
import { dialogs } from '@spiritstream/api-client';
import { toast } from '@/hooks/useToast';
import { logger } from '@/lib/logger';
import { useProfileStore } from '@/stores/profileStore';
import { useStreamStore } from '@/stores/streamStore';
import { useThemeStore } from '@/stores/themeStore';
import { useLanguageStore, type Language } from '@/stores/languageStore';
import { useHighContrast } from '@/hooks/useHighContrast';
import { incomingRtmpUrl } from '@/lib/profile-helpers';
import { testStreamConnectivity } from '@/lib/streamConnectivity';
import { cn } from '@/lib/cn';
import type { ModalName } from '@/hooks/useModalRegistry';
import type { Profile } from '@spiritstream/types';

const TRIGGER_CLASS = cn(
  'px-3 py-1.5 rounded-md text-sm font-medium text-text-secondary',
  'hover:bg-bg-hover hover:text-text-primary',
  'data-[state=open]:bg-bg-hover data-[state=open]:text-text-primary',
  'focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default',
  'transition-colors',
);

const CONTENT_CLASS = cn(
  'min-w-[220px] bg-bg-elevated border border-border-default rounded-md shadow-xl',
  'py-1 z-[var(--z-dropdown,60)]',
);

const ITEM_CLASS = cn(
  'flex items-center justify-between gap-4 px-3 py-1.5 text-sm text-text-primary',
  'cursor-pointer select-none outline-none',
  'data-[highlighted]:bg-bg-hover',
  'data-[disabled]:text-text-disabled data-[disabled]:cursor-default data-[disabled]:bg-transparent',
);

const SHORTCUT_CLASS = 'text-xs text-text-muted ms-auto ps-4';
const SEPARATOR_CLASS = 'h-px my-1 bg-border-muted';

const SUPPORTED_LANGUAGES: ReadonlyArray<{ code: Language; label: string }> = [
  { code: 'en', label: 'English' },
  { code: 'de', label: 'Deutsch' },
  { code: 'es', label: 'Español' },
  { code: 'fr', label: 'Français' },
  { code: 'af', label: 'Afrikaans' },
  { code: 'ar', label: 'العربية' },
  { code: 'ja', label: '日本語' },
  { code: 'ko', label: '한국어' },
  { code: 'ru', label: 'Русский' },
  { code: 'uk', label: 'Українська' },
  { code: 'zh-CN', label: '中文' },
];

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

export function MenuBar({
  onOpenModal,
  onToggleChat,
  chatCollapsed,
  onEditEncoder,
  canEditEncoder,
}: MenuBarProps): React.ReactElement {
  const { t, i18n } = useTranslation();
  const profileStore = useProfileStore();
  const { isStreaming, startAllGroups, stopAllGroups } = useStreamStore();
  const themes = useThemeStore((s) => s.themes);
  const currentThemeId = useThemeStore((s) => s.currentThemeId);
  const setTheme = useThemeStore((s) => s.setTheme);
  const setLanguage = useLanguageStore((s) => s.setLanguage);

  const { enabled: highContrast, toggle: toggleHighContrast } = useHighContrast();

  const handleStart = useCallback(async (): Promise<void> => {
    const current = profileStore.current;
    if (!current) return;
    try {
      await api.stream.validate(current);
      await startAllGroups(current.outputGroups, incomingRtmpUrl(current.input));
      toast.success(t('toast.streamStarted'));
    } catch (err) {
      logger.error('[menu] start failed', err);
      toast.error(
        t('toast.startFailed', { error: err instanceof Error ? err.message : String(err) }),
      );
    }
  }, [profileStore, startAllGroups, t]);

  const handleImport = useCallback(async (): Promise<void> => {
    try {
      const selected = await dialogs.openTextFile({
        multiple: false,
        filters: [{ name: 'Profile', extensions: ['json'] }],
      });
      if (!selected) return;
      const profile = JSON.parse(selected.content) as Profile;
      if (!profile.name || !profile.outputGroups) {
        throw new Error(t('errors.invalidProfileFormat'));
      }
      await api.profile.save(profile);
      await profileStore.loadProfiles();
      toast.success(t('toast.profileImported', { name: profile.name }));
    } catch (err) {
      toast.error(
        t('toast.importFailed', { error: err instanceof Error ? err.message : String(err) }),
      );
    }
  }, [profileStore, t]);

  const handleTestConnectivity = useCallback(async (): Promise<void> => {
    const current = profileStore.current;
    if (!current) {
      toast.error(t('errors.noProfileSelected'));
      return;
    }
    try {
      toast.info(t('toast.testingConnectivity', { count: 0 }));
      const result = await testStreamConnectivity(current, { enabledTargetsOnly: false });
      if (result.allPassed) {
        toast.success(t('toast.allTestsPassed', { count: result.passed }));
      } else {
        toast.error(t('toast.someTestsFailed', { passed: result.passed, failed: result.failed }));
      }
    } catch (err) {
      toast.error(
        t('toast.testFailed', { error: err instanceof Error ? err.message : String(err) }),
      );
    }
  }, [profileStore, t]);

  const handlePanic = useCallback(async (): Promise<void> => {
    try {
      const result = await api.safety.panic();
      toast.success(
        t('toast.panicStopped', {
          count: result.streamsStopped,
          ms: result.elapsedMs,
          defaultValue: 'Panic disconnect: stopped {{count}} streams in {{ms}}ms',
        }),
      );
    } catch (err) {
      toast.error(
        t('toast.panicFailed', {
          defaultValue: 'Panic disconnect failed: {{error}}',
          error: err instanceof Error ? err.message : String(err),
        }),
      );
    }
  }, [t]);

  return (
    <Menubar.Root
      className="flex items-center gap-1 px-3 h-[var(--menubar-h,40px)] bg-bg-surface border-b border-border-default"
      aria-label={t('menu.bar', { defaultValue: 'Application menu' })}
    >
      <span className="ps-1 pe-3 text-sm font-semibold text-text-primary">SpiritStream</span>

      {/* ─────────── FILE ─────────── */}
      <Menubar.Menu>
        <Menubar.Trigger className={TRIGGER_CLASS}>{t('menu.file', { defaultValue: 'File' })}</Menubar.Trigger>
        <Menubar.Portal>
          <Menubar.Content className={CONTENT_CLASS} align="start" sideOffset={4}>
            <Menubar.Item className={ITEM_CLASS} onSelect={() => onOpenModal('profileCreate')}>
              {t('menu.file.newProfile', { defaultValue: 'New profile…' })}
            </Menubar.Item>
            <Menubar.Sub>
              <Menubar.SubTrigger className={ITEM_CLASS}>
                {t('menu.file.switchProfile', { defaultValue: 'Switch profile' })}
                <ChevronRight className="w-4 h-4" />
              </Menubar.SubTrigger>
              <Menubar.Portal>
                <Menubar.SubContent className={CONTENT_CLASS}>
                  {profileStore.profiles.length === 0 ? (
                    <Menubar.Item className={ITEM_CLASS} disabled>
                      {t('menu.file.noProfiles', { defaultValue: 'No profiles yet' })}
                    </Menubar.Item>
                  ) : (
                    profileStore.profiles.map((p) => (
                      <Menubar.Item
                        key={p.name}
                        className={ITEM_CLASS}
                        onSelect={() => profileStore.selectProfile(p.name)}
                      >
                        <span>{p.name}</span>
                        {profileStore.current?.name === p.name && <Check className="w-4 h-4" />}
                      </Menubar.Item>
                    ))
                  )}
                </Menubar.SubContent>
              </Menubar.Portal>
            </Menubar.Sub>
            <Menubar.Separator className={SEPARATOR_CLASS} />
            <Menubar.Item className={ITEM_CLASS} onSelect={handleImport}>
              {t('menu.file.import', { defaultValue: 'Import profile…' })}
            </Menubar.Item>
          </Menubar.Content>
        </Menubar.Portal>
      </Menubar.Menu>

      {/* ─────────── PROFILE ─────────── */}
      <Menubar.Menu>
        <Menubar.Trigger className={TRIGGER_CLASS}>{t('menu.profile', { defaultValue: 'Profile' })}</Menubar.Trigger>
        <Menubar.Portal>
          <Menubar.Content className={CONTENT_CLASS} align="start" sideOffset={4}>
            <Menubar.Item
              className={ITEM_CLASS}
              disabled={!profileStore.current}
              onSelect={() => onOpenModal('profileEdit')}
            >
              {t('menu.profile.edit', { defaultValue: 'Edit current…' })}
            </Menubar.Item>
          </Menubar.Content>
        </Menubar.Portal>
      </Menubar.Menu>

      {/* ─────────── STREAM ─────────── */}
      <Menubar.Menu>
        <Menubar.Trigger className={TRIGGER_CLASS}>{t('menu.stream', { defaultValue: 'Stream' })}</Menubar.Trigger>
        <Menubar.Portal>
          <Menubar.Content className={CONTENT_CLASS} align="start" sideOffset={4}>
            <Menubar.Item
              className={ITEM_CLASS}
              disabled={!profileStore.current || isStreaming}
              onSelect={handleStart}
            >
              <span>{t('menu.stream.start', { defaultValue: 'Start streaming' })}</span>
              <span className={SHORTCUT_CLASS}>⌘↵</span>
            </Menubar.Item>
            <Menubar.Item className={ITEM_CLASS} disabled={!isStreaming} onSelect={() => stopAllGroups()}>
              <span>{t('menu.stream.stop', { defaultValue: 'Stop streaming' })}</span>
              <span className={SHORTCUT_CLASS}>⌘.</span>
            </Menubar.Item>
            <Menubar.Separator className={SEPARATOR_CLASS} />
            <Menubar.Item
              className={ITEM_CLASS}
              disabled={!profileStore.current || isStreaming}
              onSelect={handleTestConnectivity}
            >
              {t('menu.stream.test', { defaultValue: 'Test connectivity' })}
            </Menubar.Item>
            <Menubar.Separator className={SEPARATOR_CLASS} />
            <Menubar.Item
              className={ITEM_CLASS}
              disabled={!canEditEncoder}
              onSelect={onEditEncoder}
            >
              {t('menu.stream.encoder', { defaultValue: 'Encoder settings…' })}
            </Menubar.Item>
          </Menubar.Content>
        </Menubar.Portal>
      </Menubar.Menu>

      {/* ─────────── TOOLS ─────────── */}
      <Menubar.Menu>
        <Menubar.Trigger className={TRIGGER_CLASS}>{t('menu.tools', { defaultValue: 'Tools' })}</Menubar.Trigger>
        <Menubar.Portal>
          <Menubar.Content className={CONTENT_CLASS} align="start" sideOffset={4}>
            <Menubar.Item className={ITEM_CLASS} onSelect={() => onOpenModal('obs')}>
              {t('menu.tools.obs', { defaultValue: 'OBS connection…' })}
            </Menubar.Item>
            <Menubar.Item className={ITEM_CLASS} onSelect={() => onOpenModal('discord')}>
              {t('menu.tools.discord', { defaultValue: 'Discord notifications…' })}
            </Menubar.Item>
            <Menubar.Separator className={SEPARATOR_CLASS} />
            <Menubar.Item className={ITEM_CLASS} onSelect={() => onOpenModal('settings')}>
              <span>{t('menu.tools.settings', { defaultValue: 'Settings…' })}</span>
              <span className={SHORTCUT_CLASS}>⌘,</span>
            </Menubar.Item>
            <Menubar.Separator className={SEPARATOR_CLASS} />
            <Menubar.Item className={ITEM_CLASS} onSelect={() => onOpenModal('audit')}>
              {t('menu.tools.audit', { defaultValue: 'Audit log…' })}
            </Menubar.Item>
            <Menubar.Item className={ITEM_CLASS} onSelect={() => onOpenModal('logs')}>
              {t('menu.tools.logs', { defaultValue: 'Logs…' })}
            </Menubar.Item>
          </Menubar.Content>
        </Menubar.Portal>
      </Menubar.Menu>

      {/* ─────────── SAFETY ─────────── */}
      <Menubar.Menu>
        <Menubar.Trigger className={TRIGGER_CLASS}>{t('menu.safety', { defaultValue: 'Safety' })}</Menubar.Trigger>
        <Menubar.Portal>
          <Menubar.Content className={CONTENT_CLASS} align="start" sideOffset={4}>
            <Menubar.Item className={ITEM_CLASS} onSelect={handlePanic}>
              <span>{t('menu.safety.panic', { defaultValue: 'Panic disconnect' })}</span>
              <span className={SHORTCUT_CLASS}>⌘P</span>
            </Menubar.Item>
            <Menubar.Separator className={SEPARATOR_CLASS} />
            <Menubar.Item className={ITEM_CLASS} onSelect={() => onOpenModal('safetyWizard')}>
              {t('menu.safety.wizard', { defaultValue: 'Safety wizard…' })}
            </Menubar.Item>
          </Menubar.Content>
        </Menubar.Portal>
      </Menubar.Menu>

      {/* ─────────── VIEW ─────────── */}
      <Menubar.Menu>
        <Menubar.Trigger className={TRIGGER_CLASS}>{t('menu.view', { defaultValue: 'View' })}</Menubar.Trigger>
        <Menubar.Portal>
          <Menubar.Content className={CONTENT_CLASS} align="start" sideOffset={4}>
            <Menubar.Item className={ITEM_CLASS} onSelect={onToggleChat}>
              <span>
                {chatCollapsed
                  ? t('menu.view.showChat', { defaultValue: 'Show chat panel' })
                  : t('menu.view.hideChat', { defaultValue: 'Hide chat panel' })}
              </span>
              <span className={SHORTCUT_CLASS}>⌘\</span>
            </Menubar.Item>
            <Menubar.Separator className={SEPARATOR_CLASS} />
            <Menubar.Item className={ITEM_CLASS} onSelect={toggleHighContrast}>
              <span>{t('menu.view.highContrast', { defaultValue: 'High contrast' })}</span>
              {highContrast && <Check className="w-4 h-4" />}
            </Menubar.Item>
            <Menubar.Sub>
              <Menubar.SubTrigger className={ITEM_CLASS}>
                {t('menu.view.theme', { defaultValue: 'Theme' })}
                <ChevronRight className="w-4 h-4" />
              </Menubar.SubTrigger>
              <Menubar.Portal>
                <Menubar.SubContent className={CONTENT_CLASS}>
                  {themes.length === 0 ? (
                    <Menubar.Item className={ITEM_CLASS} disabled>
                      {t('menu.view.noThemes', { defaultValue: 'No themes installed' })}
                    </Menubar.Item>
                  ) : (
                    themes.map((theme) => (
                      <Menubar.Item
                        key={theme.id}
                        className={ITEM_CLASS}
                        onSelect={() => setTheme(theme.id)}
                      >
                        <span>{theme.name}</span>
                        {currentThemeId === theme.id && <Check className="w-4 h-4" />}
                      </Menubar.Item>
                    ))
                  )}
                </Menubar.SubContent>
              </Menubar.Portal>
            </Menubar.Sub>
            <Menubar.Sub>
              <Menubar.SubTrigger className={ITEM_CLASS}>
                {t('menu.view.language', { defaultValue: 'Language' })}
                <ChevronRight className="w-4 h-4" />
              </Menubar.SubTrigger>
              <Menubar.Portal>
                <Menubar.SubContent className={CONTENT_CLASS}>
                  {SUPPORTED_LANGUAGES.map((lang) => (
                    <Menubar.Item
                      key={lang.code}
                      className={ITEM_CLASS}
                      onSelect={() => setLanguage(lang.code)}
                    >
                      <span>{lang.label}</span>
                      {i18n.language === lang.code && <Check className="w-4 h-4" />}
                    </Menubar.Item>
                  ))}
                </Menubar.SubContent>
              </Menubar.Portal>
            </Menubar.Sub>
          </Menubar.Content>
        </Menubar.Portal>
      </Menubar.Menu>

      {/* ─────────── HELP ─────────── */}
      <Menubar.Menu>
        <Menubar.Trigger className={TRIGGER_CLASS}>{t('menu.help', { defaultValue: 'Help' })}</Menubar.Trigger>
        <Menubar.Portal>
          <Menubar.Content className={CONTENT_CLASS} align="start" sideOffset={4}>
            <Menubar.Item
              className={ITEM_CLASS}
              onSelect={() =>
                window.open(
                  'https://github.com/ScopeCreep-zip/SpiritStream',
                  '_blank',
                  'noopener,noreferrer',
                )
              }
            >
              {t('menu.help.docs', { defaultValue: 'Documentation' })}
            </Menubar.Item>
            <Menubar.Item className={ITEM_CLASS} onSelect={() => onOpenModal('shortcuts')}>
              <span>{t('menu.help.shortcuts', { defaultValue: 'Keyboard shortcuts' })}</span>
              <span className={SHORTCUT_CLASS}>⌘/</span>
            </Menubar.Item>
          </Menubar.Content>
        </Menubar.Portal>
      </Menubar.Menu>
    </Menubar.Root>
  );
}
