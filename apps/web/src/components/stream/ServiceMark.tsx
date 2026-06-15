import type { CSSProperties, ReactElement } from 'react';
import { resolveServiceLogo } from '@/lib/serviceLogos';

export interface ServiceMarkProps {
  /** Brand slug (see `serviceLogos.brandSlug`). */
  slug: string;
  /** Fallback shown when no bundled logo exists for the slug. */
  abbreviation: string;
}

/**
 * The inner mark of a service badge: a bundled Nerd Font glyph, a bundled
 * monochrome SVG (masked to the inherited text color), or the abbreviation
 * fallback. It renders ONLY the mark and inherits the parent badge's `color`,
 * so it drops into both the inline-color badges (PlatformIcon / ServiceCard)
 * and the token-colored chat badge without knowing the color source.
 */
export function ServiceMark({ slug, abbreviation }: ServiceMarkProps): ReactElement {
  const logo = resolveServiceLogo(slug);

  if (logo.kind === 'glyph') {
    return (
      <span className="service-mark-glyph" aria-hidden="true">
        {logo.char}
      </span>
    );
  }

  if (logo.kind === 'svg') {
    // The SVG path is injected as a CSS var consumed by `.service-mark-svg`
    // (the same CSS-var-injection pattern PlatformIcon uses for brand colors);
    // the class masks it to `currentColor` so it matches the badge foreground.
    return (
      <span
        className="service-mark-svg"
        aria-hidden="true"
        style={{ '--service-mark': `url("${logo.src}")` } as CSSProperties}
      />
    );
  }

  return <>{abbreviation}</>;
}
