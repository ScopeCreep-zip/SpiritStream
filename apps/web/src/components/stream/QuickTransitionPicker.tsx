/**
 * Quick Transition Picker
 * Compact dropdown for selecting transition type in Studio Mode
 */
import { useTranslation } from 'react-i18next';
import { useProfileStore } from '@/stores/profileStore';
import {
  TRANSITION_TYPES,
  getTransitionTypeLabel,
  DEFAULT_TRANSITION,
  DEFAULT_FADE_COLOR,
} from '@/types/scene';
import type { TransitionType, SceneTransition } from '@/types/scene';

export function QuickTransitionPicker() {
  const { t } = useTranslation();
  const { current: profile, updateProfile } = useProfileStore();

  const currentTransition = profile?.defaultTransition || DEFAULT_TRANSITION;
  const currentColor = currentTransition.color || DEFAULT_FADE_COLOR;

  const handleTypeChange = async (type: TransitionType) => {
    const newTransition: SceneTransition = {
      ...currentTransition,
      type,
      // Add default color when switching to fadeToColor
      ...(type === 'fadeToColor' && !currentTransition.color
        ? { color: DEFAULT_FADE_COLOR }
        : {}),
    };
    await updateProfile({ defaultTransition: newTransition });
  };

  const handleColorChange = async (color: string) => {
    const newTransition: SceneTransition = {
      ...currentTransition,
      color,
    };
    await updateProfile({ defaultTransition: newTransition });
  };

  return (
    <div className="flex flex-col items-center gap-1">
      <select
        value={currentTransition.type}
        onChange={(e) => handleTypeChange(e.target.value as TransitionType)}
        className="w-16 px-1 py-1 text-[10px] bg-[var(--bg-sunken)] border border-[var(--border-default)] rounded text-[var(--text-secondary)] text-center cursor-pointer hover:border-[var(--border-strong)] focus:outline-none focus:ring-1 focus:ring-primary/50"
        title={t('stream.transitionType', { defaultValue: 'Transition Type' })}
      >
        {TRANSITION_TYPES.map((type) => (
          <option key={type} value={type}>
            {getTransitionTypeLabel(type)}
          </option>
        ))}
      </select>
      {currentTransition.type !== 'cut' && (
        <span className="text-[9px] text-[var(--text-muted)]">
          {currentTransition.durationMs}ms
        </span>
      )}
      {currentTransition.type === 'fadeToColor' && (
        <div className="flex items-center gap-1">
          <input
            type="color"
            value={currentColor}
            onChange={(e) => handleColorChange(e.target.value)}
            className="w-5 h-5 rounded border border-[var(--border-default)] cursor-pointer p-0"
            title={t('stream.transitionColor', { defaultValue: 'Transition Color' })}
          />
          <span className="text-[9px] text-[var(--text-muted)] uppercase">
            {currentColor.replace('#', '')}
          </span>
        </div>
      )}
    </div>
  );
}
