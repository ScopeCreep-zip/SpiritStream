import { create } from 'zustand';
import i18n from '@/lib/i18n';

export type Language = 'en' | 'es' | 'fr' | 'de' | 'ja' | 'ar' | 'zh-CN' | 'ko' | 'uk' | 'ru' | 'af';

const rtlLanguages = new Set<Language>(['ar']);

const applyLanguage = (lang: Language) => {
  i18n.changeLanguage(lang);
  if (typeof document !== 'undefined') {
    document.documentElement.lang = lang;
    document.documentElement.dir = rtlLanguages.has(lang) ? 'rtl' : 'ltr';
  }
};

interface LanguageStore {
  language: Language;
  setLanguage: (lang: Language) => void;
  initFromSettings: (lang: string) => void;
}

export const useLanguageStore = create<LanguageStore>((set) => ({
  language: 'en',

  setLanguage: (lang: Language) => {
    applyLanguage(lang);
    set({ language: lang });
  },

  initFromSettings: (lang: string) => {
    // Validate the language is supported
    const supportedLanguages: Language[] = [
      'en',
      'es',
      'fr',
      'de',
      'ja',
      'ar',
      'zh-CN',
      'ko',
      'uk',
      'ru',
      'af',
    ];
    const validLang = supportedLanguages.includes(lang as Language) ? (lang as Language) : 'en';

    applyLanguage(validLang);
    set({ language: validLang });
  },
}));
