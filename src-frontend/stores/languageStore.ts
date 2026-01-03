import { create } from 'zustand';
import i18n from '@/lib/i18n';

export type Language = 'en' | 'es' | 'fr' | 'de' | 'ja';

interface LanguageStore {
  language: Language;
  setLanguage: (lang: Language) => void;
  initFromSettings: (lang: string) => void;
}

export const useLanguageStore = create<LanguageStore>((set) => ({
  language: 'en',

  setLanguage: (lang: Language) => {
    i18n.changeLanguage(lang);
    set({ language: lang });
  },

  initFromSettings: (lang: string) => {
    // Validate the language is supported
    const supportedLanguages: Language[] = ['en', 'es', 'fr', 'de', 'ja'];
    const validLang = supportedLanguages.includes(lang as Language)
      ? (lang as Language)
      : 'en';

    i18n.changeLanguage(validLang);
    set({ language: validLang });
  },
}));
