import type { Platform } from '@spiritstream/types';
import { PLATFORMS } from '@/types/generated-platforms';

export type ServiceCapability = 'rtmp' | 'chat';

export interface ServiceEntry {
  readonly platform: Platform;
  readonly displayName: string;
  readonly abbreviation: string;
  readonly color: string;
  readonly textColor: string;
  readonly capabilities: ReadonlyArray<ServiceCapability>;
}

export type ServiceCategoryId = 'popular' | 'adult' | 'regional' | 'tools' | 'custom' | 'more';

export interface ServiceCategory {
  readonly id: ServiceCategoryId;
  /** Optional translation-key for a small grey hint next to the category header. */
  readonly hint?: string;
  readonly entries: ReadonlyArray<ServiceEntry>;
}

/**
 * Curated lists. The "More" category absorbs every platform from the
 * generated `PLATFORMS` object that isn't named here, so the catalog stays
 * exhaustive even as Rust adds platforms — no silent drops.
 */
const POPULAR: ReadonlyArray<Platform> = [
  'Twitch',
  'YouTube - RTMPS',
  'Kick',
  'Rumble',
  'Facebook Live',
  'TikTok Live',
  'Trovo',
  'DLive',
];

const ADULT: ReadonlyArray<Platform> = [
  'Chaturbate',
  'Stripchat',
  'CAM4',
  'Bongacams',
  'OnlyFans.com',
  'MyFreeCams',
  'Lovecast',
  'XLoveCam.com',
];

const REGIONAL: ReadonlyArray<Platform> = [
  'niconico (ニコニコ生放送)',
  'SOOP Korea',
  'SOOP Global',
  'Bilibili Live - RTMP | 哔哩哔哩直播 - RTMP',
  'CHZZK',
  'KakaoTV',
  'GoodGame.ru',
  'PandaTV | 판더티비',
  'Kuaishou Live',
];

const TOOLS: ReadonlyArray<Platform> = [
  'Restream.io',
  'Streamlabs',
  'Switchboard Live',
  'Castr.io',
  'IRLToolkit',
  'Mux',
  'Livepeer Studio',
];

/**
 * Platforms that support chat (OAuth bot / API). The AppDrawer filters by
 * capability when invoked for chat-source connection.
 *
 * Conservatively scoped: only platforms with a documented chat ingestion
 * path. SpiritStream backend wires Twitch / YouTube / Kick / Discord chat;
 * the rest are RTMP-only.
 */
const CHAT_CAPABLE: ReadonlySet<Platform> = new Set<Platform>([
  'Twitch',
  'YouTube - RTMPS',
  'Kick',
]);

function entryFor(platform: Platform): ServiceEntry {
  const cfg = PLATFORMS[platform];
  const capabilities: ServiceCapability[] = ['rtmp'];
  if (CHAT_CAPABLE.has(platform)) capabilities.push('chat');
  return {
    platform,
    displayName: cfg.displayName,
    abbreviation: cfg.abbreviation,
    color: cfg.color,
    textColor: cfg.textColor,
    capabilities,
  };
}

/**
 * Build the categorized catalog from the generated PLATFORMS object.
 * Every Platform value lands in exactly one category — explicit category
 * lists win; everything else falls into "More" so nothing is silently lost.
 */
export function buildServiceCatalog(): ReadonlyArray<ServiceCategory> {
  const allPlatforms = Object.keys(PLATFORMS) as Platform[];
  const claimed = new Set<Platform>([...POPULAR, ...ADULT, ...REGIONAL, ...TOOLS, 'Custom']);
  const remaining = allPlatforms
    .filter((p) => !claimed.has(p))
    .sort((a, b) => PLATFORMS[a].displayName.localeCompare(PLATFORMS[b].displayName));

  return [
    { id: 'popular', entries: POPULAR.map(entryFor) },
    { id: 'adult', hint: 'drawer.hint.adultE2E', entries: ADULT.map(entryFor) },
    { id: 'regional', entries: REGIONAL.map(entryFor) },
    { id: 'tools', entries: TOOLS.map(entryFor) },
    { id: 'custom', entries: [entryFor('Custom')] },
    { id: 'more', entries: remaining.map(entryFor) },
  ];
}

export function filterCatalog(
  catalog: ReadonlyArray<ServiceCategory>,
  query: string,
  capability: ServiceCapability
): ReadonlyArray<ServiceCategory> {
  const needle = query.trim().toLowerCase();
  return catalog
    .map((cat) => ({
      ...cat,
      entries: cat.entries.filter((e) => {
        if (!e.capabilities.includes(capability)) return false;
        if (needle === '') return true;
        return (
          e.displayName.toLowerCase().includes(needle) ||
          e.platform.toLowerCase().includes(needle) ||
          e.abbreviation.toLowerCase().includes(needle)
        );
      }),
    }))
    .filter((cat) => cat.entries.length > 0);
}
