/**
 * AddFilterDropdown — Dropdown menu for adding new audio filters
 * Extracted from AudioFilterChainEditor
 */
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus } from 'lucide-react';
import type { AudioFilterType } from '@/types/source';
import { getAudioFilterLabel } from '@/types/source';

interface AddFilterDropdownProps {
  onAdd: (type: AudioFilterType) => void;
}

export function AddFilterDropdown({ onAdd }: AddFilterDropdownProps) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);

  const filterTypes: AudioFilterType[] = ['gain', 'compressor', 'noiseGate', 'noiseSuppression', 'expander'];

  return (
    <div className="relative">
      <button
        type="button"
        className="flex items-center gap-1 text-[10px] text-[var(--text-muted)] hover:text-[var(--text-secondary)] transition-colors"
        onClick={() => setOpen(!open)}
      >
        <Plus className="w-3 h-3" />
        {t('audio.addFilter', { defaultValue: 'Add Filter' })}
      </button>
      {open && (
        <>
          <div className="fixed inset-0 z-10" onClick={() => setOpen(false)} />
          <div className="absolute right-0 top-full mt-1 z-20 bg-[var(--bg-elevated)] border border-[var(--border-default)] rounded-md shadow-lg min-w-[140px]">
            {filterTypes.map((type) => (
              <button
                key={type}
                type="button"
                className="w-full text-left px-3 py-1.5 text-[11px] text-[var(--text-secondary)] hover:bg-[var(--bg-sunken)] transition-colors first:rounded-t-md last:rounded-b-md"
                onClick={() => { onAdd(type); setOpen(false); }}
              >
                {getAudioFilterLabel(type)}
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
