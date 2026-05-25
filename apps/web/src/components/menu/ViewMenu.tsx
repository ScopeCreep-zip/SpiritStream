import React from 'react';
import * as Menubar from '@radix-ui/react-menubar';
import { useTranslation } from 'react-i18next';
import { Check, ChevronRight } from 'lucide-react';
import { useThemeStore } from '@/stores/themeStore';
import { useLanguageStore } from '@/stores/languageStore';
import { useHighContrast } from '@/hooks/useHighContrast';
import {
  TRIGGER_CLASS,
  CONTENT_CLASS,
  ITEM_CLASS,
  SHORTCUT_CLASS,
  SEPARATOR_CLASS,
  SUPPORTED_LANGUAGES,
} from './menuStyles';

interface ViewMenuProps {
  /** Toggle the chat column collapsed/expanded state. */
  onToggleChat: () => void;
  chatCollapsed: boolean;
}

export function ViewMenu({ onToggleChat, chatCollapsed }: ViewMenuProps): React.ReactElement {
  const { t, i18n } = useTranslation();
  const themes = useThemeStore((s) => s.themes);
  const currentThemeId = useThemeStore((s) => s.currentThemeId);
  const setTheme = useThemeStore((s) => s.setTheme);
  const setLanguage = useLanguageStore((s) => s.setLanguage);
  const { enabled: highContrast, toggle: toggleHighContrast } = useHighContrast();

  return (
    <Menubar.Menu>
      <Menubar.Trigger className={TRIGGER_CLASS}>
        {t('menu.view', { defaultValue: 'View' })}
      </Menubar.Trigger>
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
  );
}
