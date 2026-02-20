import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';

import en from '@/locales/en.json';

// Only English is bundled eagerly. Other locales are loaded on demand.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const localeImporters: Record<string, () => Promise<{ default: Record<string, any> }>> = {
  es: () => import('@/locales/es.json'),
  fr: () => import('@/locales/fr.json'),
  de: () => import('@/locales/de.json'),
  ja: () => import('@/locales/ja.json'),
  ar: () => import('@/locales/ar.json'),
  'zh-CN': () => import('@/locales/zh-CN.json'),
  ko: () => import('@/locales/ko.json'),
  uk: () => import('@/locales/uk.json'),
  ru: () => import('@/locales/ru.json'),
  af: () => import('@/locales/af.json'),
};

i18n.use(initReactI18next).init({
  resources: {
    en: { translation: en },
  },
  lng: 'en',
  fallbackLng: 'en',
  interpolation: {
    escapeValue: false, // React already escapes
  },
});

/** Change the active language, lazy-loading the locale bundle if needed. */
export async function changeLanguage(lng: string): Promise<void> {
  if (lng === 'en') {
    await i18n.changeLanguage('en');
    return;
  }

  // Load locale on demand if not already loaded
  if (!i18n.hasResourceBundle(lng, 'translation')) {
    const importer = localeImporters[lng];
    if (importer) {
      const mod = await importer();
      i18n.addResourceBundle(lng, 'translation', mod.default, true, true);
    }
  }

  await i18n.changeLanguage(lng);
}

export default i18n;
