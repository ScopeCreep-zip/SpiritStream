import { describe, it, expect, beforeEach, vi } from 'vitest';

// N5: language store. Its load-bearing logic is (1) rejecting an
// unsupported locale back to 'en' so a corrupt settings file can't wedge
// the UI in an undefined language, and (2) flipping `document.dir` to
// `rtl` for Arabic — the only RTL locale. A regression in either is an
// accessibility break for the populations SpiritStream protects.

const changeLanguage = vi.fn();
vi.mock('@/lib/i18n', () => ({
  default: { changeLanguage: (lang: string) => changeLanguage(lang) },
}));

import { useLanguageStore } from './languageStore';

beforeEach(() => {
  changeLanguage.mockClear();
  useLanguageStore.setState({ language: 'en' });
  document.documentElement.lang = '';
  document.documentElement.dir = '';
});

describe('languageStore.setLanguage', () => {
  it('applies a supported language and updates document attributes', () => {
    useLanguageStore.getState().setLanguage('de');
    expect(useLanguageStore.getState().language).toBe('de');
    expect(changeLanguage).toHaveBeenCalledWith('de');
    expect(document.documentElement.lang).toBe('de');
    expect(document.documentElement.dir).toBe('ltr');
  });

  it('sets dir=rtl for Arabic', () => {
    useLanguageStore.getState().setLanguage('ar');
    expect(document.documentElement.dir).toBe('rtl');
  });
});

describe('languageStore.initFromSettings', () => {
  it('accepts a supported locale string', () => {
    useLanguageStore.getState().initFromSettings('ja');
    expect(useLanguageStore.getState().language).toBe('ja');
    expect(changeLanguage).toHaveBeenCalledWith('ja');
  });

  it('falls back to en for an unsupported locale string', () => {
    useLanguageStore.getState().initFromSettings('xx-not-a-locale');
    expect(useLanguageStore.getState().language).toBe('en');
    expect(changeLanguage).toHaveBeenCalledWith('en');
    expect(document.documentElement.dir).toBe('ltr');
  });
});
