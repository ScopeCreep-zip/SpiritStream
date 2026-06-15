/**
 * Official-logo resolution for streaming services. Three tiers, in priority
 * order, so coverage degrades cleanly:
 *
 *  1. `glyph` — a Nerd Font glyph from the bundled Symbols Nerd Font subset
 *     (`/fonts/SymbolsNerdFont-Brands-Subset.woff2`). Only the brands the font
 *     actually ships (Twitch, YouTube, Facebook).
 *  2. `svg`   — a bundled monochrome SVG mark (Simple Icons, CC0) under
 *     `/icons/platforms/<slug>.svg`.
 *  3. `none`  — no bundled logo; the caller renders the colored abbreviation
 *     badge it already had.
 *
 * Everything is bundled in the repo (no network) — this is a local app. Both
 * logo tiers render as a single-color mark in the badge's foreground color, so
 * they sit inside the existing colored badge exactly where the abbreviation
 * used to, keeping one visual language.
 *
 * Keyed by a normalized brand slug. Chat platform ids are already slugs
 * (`twitch`, `kick`, …); catalog `Platform`s are slugified from their
 * `displayName` via {@link brandSlug} (the raw Platform name "TikTok Live"
 * would not match, but its displayName "TikTok" does).
 */

/** Nerd Font (Symbols Nerd Font) PUA glyphs, by brand slug. */
const GLYPHS: Readonly<Record<string, string>> = {
  twitch: '\u{f1e8}',
  youtube: '\u{f16a}',
  facebook: '\u{f09a}',
};

/** Brands with a bundled monochrome SVG at `/icons/platforms/<slug>.svg`. */
const SVG_BRANDS: ReadonlySet<string> = new Set([
  'kick',
  'rumble',
  'tiktok',
  'bilibili',
  'niconico',
  'onlyfans',
  'streamlabs',
]);

export type ServiceLogo =
  | { readonly kind: 'glyph'; readonly char: string }
  | { readonly kind: 'svg'; readonly src: string }
  | { readonly kind: 'none' };

/** Normalize a service/platform display name to a brand slug (lowercase
 *  alphanumerics; parentheticals like "(Backup)" dropped first). */
export function brandSlug(name: string): string {
  // Drop any trailing parenthetical ("YouTube (Backup)", "niconico (…)") by
  // cutting at the first '(', then collapse to lowercase alphanumerics.
  return name.toLowerCase().split('(')[0].replace(/[^a-z0-9]/g, '');
}

/** Resolve the bundled logo for a brand slug, or `none` to fall back to the
 *  abbreviation badge. */
export function resolveServiceLogo(slug: string): ServiceLogo {
  const glyph = GLYPHS[slug];
  if (glyph) return { kind: 'glyph', char: glyph };
  if (SVG_BRANDS.has(slug)) return { kind: 'svg', src: `/icons/platforms/${slug}.svg` };
  return { kind: 'none' };
}
