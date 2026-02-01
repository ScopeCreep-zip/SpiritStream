import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';

import en from '@/locales/en.json';
import es from '@/locales/es.json';
import fr from '@/locales/fr.json';
import de from '@/locales/de.json';
import ja from '@/locales/ja.json';
import ar from '@/locales/ar.json';
import zhCN from '@/locales/zh-CN.json';
import ko from '@/locales/ko.json';
import uk from '@/locales/uk.json';
import ru from '@/locales/ru.json';
import af from '@/locales/af.json';

i18n.use(initReactI18next).init({
  resources: {
    en: { translation: en },
    es: { translation: es },
    fr: { translation: fr },
    de: { translation: de },
    ja: { translation: ja },
    ar: { translation: ar },
    'zh-CN': { translation: zhCN },
    ko: { translation: ko },
    uk: { translation: uk },
    ru: { translation: ru },
    af: { translation: af },
  },
  lng: 'en',
  fallbackLng: 'en',
  interpolation: {
    escapeValue: false, // React already escapes
  },
});

export default i18n;
