import React, { useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus } from 'lucide-react';
import { cn } from '@/lib/cn';
import type { OutputGroup } from '@spiritstream/types';

interface GroupTabsProps {
  groups: ReadonlyArray<OutputGroup>;
  activeGroupId: string | null;
  onSelectGroup: (id: string) => void;
  onAddGroup: () => void;
  disabled?: boolean;
}

/**
 * Horizontal tab strip for selecting between output groups. Implements the
 * WAI-ARIA tabs pattern: role=tablist, role=tab on each pill, arrow-key
 * navigation, Home/End jumps. The corresponding panel (rendered separately
 * by the parent) carries role=tabpanel + aria-labelledby pointing at the
 * active tab's id.
 */
export function GroupTabs({
  groups,
  activeGroupId,
  onSelectGroup,
  onAddGroup,
  disabled,
}: GroupTabsProps): React.ReactElement {
  const { t } = useTranslation();
  const tabRefs = useRef<Map<string, HTMLButtonElement>>(new Map());

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLDivElement>) => {
      if (groups.length === 0) return;
      const ids = groups.map((g) => g.id);
      const currentIndex = activeGroupId ? ids.indexOf(activeGroupId) : 0;
      let nextIndex: number | null = null;

      switch (e.key) {
        case 'ArrowRight':
          nextIndex = (currentIndex + 1) % ids.length;
          break;
        case 'ArrowLeft':
          nextIndex = (currentIndex - 1 + ids.length) % ids.length;
          break;
        case 'Home':
          nextIndex = 0;
          break;
        case 'End':
          nextIndex = ids.length - 1;
          break;
      }

      if (nextIndex !== null) {
        e.preventDefault();
        const nextId = ids[nextIndex];
        onSelectGroup(nextId);
        tabRefs.current.get(nextId)?.focus();
      }
    },
    [groups, activeGroupId, onSelectGroup],
  );

  const hasGroups = groups.length > 0;
  const addButton = (
    <button
      type="button"
      onClick={onAddGroup}
      disabled={disabled}
      aria-label={t('pipeline.groups.add', { defaultValue: 'New output group' })}
      className={cn(
        'inline-flex items-center gap-1 px-3 h-9 rounded-md text-sm',
        'border border-dashed border-border-default text-text-tertiary',
        'hover:border-border-strong hover:text-text-primary hover:bg-bg-hover',
        'focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default',
        'disabled:opacity-50 disabled:cursor-not-allowed',
        'transition-colors',
      )}
    >
      <Plus className="w-4 h-4" aria-hidden="true" />
      {t('pipeline.groups.new', { defaultValue: 'New group' })}
    </button>
  );

  // role="tablist" requires role="tab" children only — the Add button is
  // rendered outside the tablist as a sibling. When there are no groups,
  // we drop the tablist semantics entirely (an empty tablist is invalid).
  if (!hasGroups) {
    return <div className="flex items-center gap-2 flex-wrap">{addButton}</div>;
  }

  return (
    <div className="flex items-center gap-2 flex-wrap">
      <div
        role="tablist"
        aria-label={t('pipeline.groups.label', { defaultValue: 'Output groups' })}
        onKeyDown={handleKeyDown}
        className="flex items-center gap-2 flex-wrap"
      >
        {groups.map((g) => {
          const active = g.id === activeGroupId;
          return (
            <button
              key={g.id}
              ref={(el) => {
                if (el) tabRefs.current.set(g.id, el);
                else tabRefs.current.delete(g.id);
              }}
              type="button"
              role="tab"
              id={`group-tab-${g.id}`}
              aria-selected={active}
              aria-controls={`group-panel-${g.id}`}
              tabIndex={active ? 0 : -1}
              onClick={() => onSelectGroup(g.id)}
              className={cn(
                'inline-flex items-center gap-2 px-3 h-9 rounded-md text-sm font-medium transition-colors',
                'focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default',
                active
                  ? 'bg-primary text-primary-foreground'
                  : 'bg-bg-muted text-text-secondary hover:bg-bg-hover hover:text-text-primary',
              )}
            >
              <span>{g.name}</span>
              <span className={cn('text-xs', active ? 'text-primary-foreground/80' : 'text-text-tertiary')}>
                {g.video.height}p{g.video.fps}
              </span>
            </button>
          );
        })}
      </div>
      {addButton}
    </div>
  );
}
