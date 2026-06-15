import React, { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Search } from 'lucide-react';
import { Modal } from '@/components/ui/Modal';
import { cn } from '@/lib/cn';
import {
  buildServiceCatalog,
  filterCatalog,
  type ServiceCapability,
  type ServiceCategoryId,
  type ServiceEntry,
} from './serviceCatalog';
import type { Platform } from '@spiritstream/types';
import { ServiceMark } from '@/components/stream/ServiceMark';
import { brandSlug } from '@/lib/serviceLogos';

interface AppDrawerProps {
  open: boolean;
  onClose: () => void;
  /**
   * 'stream-target' — show RTMP-capable platforms; selection hands off to
   * `onSelect` so the caller can open TargetModal pre-filled with the choice.
   * 'chat-source' — restrict to platforms with chat ingestion.
   */
  mode: 'stream-target' | 'chat-source';
  onSelect: (platform: Platform) => void;
}

const CATEGORY_LABEL: Record<ServiceCategoryId, string> = {
  popular: 'drawer.category.popular',
  adult: 'drawer.category.adult',
  regional: 'drawer.category.regional',
  tools: 'drawer.category.tools',
  custom: 'drawer.category.custom',
  more: 'drawer.category.more',
};

const CATEGORY_FALLBACK: Record<ServiceCategoryId, string> = {
  popular: 'Popular',
  adult: 'Adult & cam',
  regional: 'Regional',
  tools: 'Tools & restream',
  custom: 'Custom',
  more: 'More',
};

export function AppDrawer({ open, onClose, mode, onSelect }: AppDrawerProps): React.ReactElement {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);

  const catalog = useMemo(() => buildServiceCatalog(), []);
  const capability: ServiceCapability = mode === 'chat-source' ? 'chat' : 'rtmp';
  const filtered = useMemo(
    () => filterCatalog(catalog, query, capability),
    [catalog, query, capability]
  );

  // Reset the query whenever the drawer opens — selecting a service should
  // not leave a stale filter behind for the next opening.
  useEffect(() => {
    if (open) setQuery('');
  }, [open]);

  const handleSelect = (entry: ServiceEntry): void => {
    onClose();
    onSelect(entry.platform);
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={t(mode === 'chat-source' ? 'drawer.title.chat' : 'drawer.title.stream', {
        defaultValue: mode === 'chat-source' ? 'Connect chat source' : 'Add streaming destination',
      })}
      maxWidth="720px"
      closeOnBackdropClick
    >
      <div className="relative mb-4">
        <Search
          className="absolute start-3 top-1/2 -translate-y-1/2 w-4 h-4 text-text-tertiary"
          aria-hidden="true"
        />
        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder={t('drawer.searchPlaceholder', {
            defaultValue: 'Search platforms…',
          })}
          aria-label={t('drawer.searchLabel', { defaultValue: 'Search platforms' })}
          className={cn(
            'w-full h-10 ps-9 pe-3 rounded-md text-sm',
            'bg-bg-sunken text-text-primary border border-border-default',
            'placeholder:text-text-muted',
            'focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default'
          )}
        />
      </div>

      <div
        className="max-h-[60vh] overflow-y-auto pe-1"
        role="list"
        aria-label={t('drawer.resultsLabel', { defaultValue: 'Available platforms' })}
      >
        {filtered.length === 0 ? (
          <p className="py-12 text-center text-sm text-text-secondary">
            {t('drawer.noResults', {
              defaultValue: 'No platforms match "{{q}}".',
              q: query,
            })}
          </p>
        ) : (
          filtered.map((category) => (
            <section
              key={category.id}
              role="group"
              aria-labelledby={`drawer-cat-${category.id}`}
              className="mb-6 last:mb-0"
            >
              <header className="flex items-baseline gap-2 mb-2">
                <h3
                  id={`drawer-cat-${category.id}`}
                  className="text-xs font-semibold text-text-secondary uppercase tracking-wide"
                >
                  {t(CATEGORY_LABEL[category.id], {
                    defaultValue: CATEGORY_FALLBACK[category.id],
                  })}
                </h3>
                {category.hint && (
                  <span className="text-xs text-text-muted">
                    {t(category.hint, { defaultValue: '' })}
                  </span>
                )}
              </header>
              <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 gap-2">
                {category.entries.map((entry) => (
                  <ServiceCard
                    key={entry.platform}
                    entry={entry}
                    onSelect={() => handleSelect(entry)}
                  />
                ))}
              </div>
            </section>
          ))
        )}
      </div>
    </Modal>
  );
}

interface ServiceCardProps {
  entry: ServiceEntry;
  onSelect: () => void;
}

function ServiceCard({ entry, onSelect }: ServiceCardProps): React.ReactElement {
  return (
    <button
      type="button"
      onClick={onSelect}
      className={cn(
        'flex items-center gap-2 px-3 py-2 rounded-md text-start',
        'bg-bg-surface border border-border-muted',
        'hover:border-primary hover:bg-bg-hover',
        'focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default',
        'transition-colors'
      )}
    >
      <span
        aria-hidden="true"
        className="flex-shrink-0 inline-flex items-center justify-center w-8 h-8 rounded text-xs font-semibold bg-[var(--service-bg)] text-[var(--service-fg)]"
        // Per-service brand colors are config data, not static tokens — inject
        // as CSS vars (Modal.tsx pattern).
        style={{ '--service-bg': entry.color, '--service-fg': entry.textColor } as React.CSSProperties}
      >
        <ServiceMark slug={brandSlug(entry.displayName)} abbreviation={entry.abbreviation} />
      </span>
      <span className="flex flex-col min-w-0">
        <span className="text-sm font-medium text-text-primary truncate">{entry.displayName}</span>
        <span className="text-xs text-text-tertiary truncate">
          {entry.capabilities.join(' · ')}
        </span>
      </span>
    </button>
  );
}
