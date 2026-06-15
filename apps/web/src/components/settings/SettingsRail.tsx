import React, { useCallback, useRef } from 'react';
import { cn } from '@/lib/cn';
import type { SettingsSection } from '@/hooks/useModalRegistry';

export interface RailItem {
  readonly id: SettingsSection;
  readonly label: string;
}

interface SettingsRailProps {
  items: ReadonlyArray<RailItem>;
  active: SettingsSection;
  onSelect: (id: SettingsSection) => void;
  ariaLabel: string;
}

/**
 * Vertical tab-rail for the unified settings window. WAI-ARIA tabs pattern
 * (vertical orientation) adapted from the horizontal `pipeline/GroupTabs`:
 * roving tabindex, ArrowUp/Down to move + select (selection follows focus),
 * Home/End jumps. The single content region carries role=tabpanel +
 * aria-labelledby pointing at the active tab.
 */
export function SettingsRail({
  items,
  active,
  onSelect,
  ariaLabel,
}: SettingsRailProps): React.ReactElement {
  const tabRefs = useRef<Map<SettingsSection, HTMLButtonElement>>(new Map());

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLDivElement>) => {
      const ids = items.map((i) => i.id);
      if (ids.length === 0) return;
      const current = Math.max(0, ids.indexOf(active));
      let next: number | null = null;
      switch (e.key) {
        case 'ArrowDown':
          next = (current + 1) % ids.length;
          break;
        case 'ArrowUp':
          next = (current - 1 + ids.length) % ids.length;
          break;
        case 'Home':
          next = 0;
          break;
        case 'End':
          next = ids.length - 1;
          break;
      }
      if (next !== null) {
        e.preventDefault();
        const id = ids[next];
        onSelect(id);
        tabRefs.current.get(id)?.focus();
      }
    },
    [items, active, onSelect]
  );

  return (
    <div
      role="tablist"
      aria-orientation="vertical"
      aria-label={ariaLabel}
      onKeyDown={handleKeyDown}
      className="flex flex-col gap-1 w-48 shrink-0 border-e border-border-muted pe-2 overflow-y-auto"
    >
      {items.map((item) => {
        const selected = item.id === active;
        return (
          <button
            key={item.id}
            ref={(el) => {
              if (el) tabRefs.current.set(item.id, el);
              else tabRefs.current.delete(item.id);
            }}
            type="button"
            role="tab"
            id={`settings-tab-${item.id}`}
            aria-selected={selected}
            aria-controls={`settings-panel-${item.id}`}
            tabIndex={selected ? 0 : -1}
            onClick={() => onSelect(item.id)}
            className={cn(
              'text-start px-3 py-2 rounded-md text-sm font-medium transition-colors',
              'focus-visible:outline-none focus-visible:ring-[3px] focus-visible:ring-ring-default',
              selected
                ? 'bg-primary text-primary-foreground'
                : 'text-text-secondary hover:bg-bg-hover hover:text-text-primary'
            )}
          >
            {item.label}
          </button>
        );
      })}
    </div>
  );
}
