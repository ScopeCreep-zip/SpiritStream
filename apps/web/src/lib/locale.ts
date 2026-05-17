/**
 * Locale-aware formatting helpers.
 *
 * Wraps `Intl.DateTimeFormat` and `Intl.NumberFormat` so the rest of
 * the app doesn't sprinkle locale lookups across components.
 * The active language comes from `i18next` (which is in sync with
 * `document.documentElement.lang`). Each helper accepts an explicit
 * `locale` argument for tests / Storybook contexts.
 */

import i18n from '@/lib/i18n';

/** The locale tag i18next is currently using (e.g. `"en"`, `"ar"`, `"zh-CN"`). */
export function currentLocale(): string {
  return i18n.language || 'en';
}

/** Format a Date or epoch-millis as "Sep 14, 2:30 PM" in the active locale. */
export function formatDateTime(value: Date | number | string, locale = currentLocale()): string {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return '';
  return new Intl.DateTimeFormat(locale, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(date);
}

/** Format a Date as a calendar date only ("Sep 14, 2026"). */
export function formatDate(value: Date | number | string, locale = currentLocale()): string {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return '';
  return new Intl.DateTimeFormat(locale, { dateStyle: 'medium' }).format(date);
}

/** Format a Date as a clock time only ("2:30 PM" / "14:30"). */
export function formatTime(value: Date | number | string, locale = currentLocale()): string {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return '';
  return new Intl.DateTimeFormat(locale, { timeStyle: 'short' }).format(date);
}

/**
 * Format a count of bytes ("1.2 MB", "456 KB", "12 B") using
 * `Intl.NumberFormat` for the localised decimal separator.
 */
export function formatBytes(bytes: number, locale = currentLocale()): string {
  if (!Number.isFinite(bytes) || bytes < 0) return '';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let n = bytes;
  let i = 0;
  while (n >= 1024 && i < units.length - 1) {
    n /= 1024;
    i += 1;
  }
  const fmt = new Intl.NumberFormat(locale, {
    maximumFractionDigits: i === 0 ? 0 : 1,
  });
  return `${fmt.format(n)} ${units[i]}`;
}

/**
 * Format a stream bitrate. Input is in **kbps**. Output is "1,234 kbps"
 * or "1.5 Mbps" depending on magnitude, with localised separators.
 */
export function formatBitrate(kbps: number, locale = currentLocale()): string {
  if (!Number.isFinite(kbps) || kbps < 0) return '';
  if (kbps >= 1000) {
    const fmt = new Intl.NumberFormat(locale, { maximumFractionDigits: 1 });
    return `${fmt.format(kbps / 1000)} Mbps`;
  }
  return `${new Intl.NumberFormat(locale).format(Math.round(kbps))} kbps`;
}

/**
 * Format an uptime span ("1d 4h 12m", "12m 3s") in the active locale.
 * Locale-aware in that the numeric separators come from
 * `Intl.NumberFormat`; the unit labels are translation-keys in the
 * i18n catalog so callers can swap them out per locale.
 */
export function formatUptime(seconds: number, locale = currentLocale()): string {
  if (!Number.isFinite(seconds) || seconds < 0) return '';
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.floor(seconds % 60);
  const fmt = new Intl.NumberFormat(locale);
  const parts: string[] = [];
  if (d > 0) parts.push(`${fmt.format(d)}d`);
  if (h > 0 || d > 0) parts.push(`${fmt.format(h)}h`);
  if (m > 0 || h > 0 || d > 0) parts.push(`${fmt.format(m)}m`);
  parts.push(`${fmt.format(s)}s`);
  return parts.join(' ');
}

/**
 * Format a plain count with localised thousands separators
 * ("1,234" / "1.234" / "١٬٢٣٤"). For plural-aware text use i18next's
 * `t(key, { count })` directly — i18next's typed-keys setup needs
 * literal keys at the call site to typecheck, which means a generic
 * helper can't accept arbitrary string keys.
 */
export function formatCount(count: number, locale = currentLocale()): string {
  if (!Number.isFinite(count)) return '';
  return new Intl.NumberFormat(locale).format(count);
}
