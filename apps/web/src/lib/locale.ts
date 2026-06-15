/**
 * Locale-aware formatting helpers.
 *
 * Wraps `Intl.DateTimeFormat` so the rest of the app doesn't sprinkle
 * locale lookups across components. The active language comes from
 * `i18next` (which is in sync with `document.documentElement.lang`).
 * Each helper accepts an explicit `locale` argument for tests / Storybook
 * contexts.
 */

import i18n from '@/lib/i18n';

/** The locale tag i18next is currently using (e.g. `"en"`, `"ar"`, `"zh-CN"`). */
export function currentLocale(): string {
  return i18n.language || 'en';
}

/** Accepted timestamp inputs across the codebase: native `Date`, epoch
 * milliseconds, or an ISO-8601 string. */
export type TimestampInput = Date | number | string;

/** Format a Date or epoch-millis as "Sep 14, 2:30 PM" in the active locale. */
export function formatDateTime(value: TimestampInput, locale = currentLocale()): string {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return '';
  return new Intl.DateTimeFormat(locale, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(date);
}
