/**
 * Scene Properties Section
 * Extracted from PropertiesPanel — shown when no layer is selected
 * Allows editing scene name, canvas info, and transition settings
 */
import { useState, useEffect } from 'react';
import type { TFunction } from 'i18next';
import { Monitor } from 'lucide-react';
import { Input } from '@/components/ui/Input';
import { Select } from '@/components/ui/Select';
import type { Profile, Scene, SceneTransition, TransitionType } from '@/types/profile';
import { TRANSITION_TYPES, getTransitionTypeLabel, DEFAULT_TRANSITION } from '@/types/scene';
import { useSceneStore } from '@/stores/sceneStore';
import { useProfileStore } from '@/stores/profileStore';
import { api } from '@/lib/backend';
import { toast, formatError } from '@/hooks/useToast';
import { blurOnEnter } from '@/utils/inputHandlers';

interface ScenePropertiesSectionProps {
  profile: Profile;
  scene: Scene;
  t: TFunction;
}

export function ScenePropertiesSection({ profile, scene, t }: ScenePropertiesSectionProps) {
  const { updateCurrentScene, updateCurrentProfile } = useProfileStore();
  const { updateScene } = useSceneStore();

  const [sceneName, setSceneName] = useState(scene.name);

  // Sync local state with scene
  useEffect(() => {
    setSceneName(scene.name);
  }, [scene.name]);

  // Get effective transition (scene override or profile default)
  const effectiveTransition = scene.transitionIn || profile.defaultTransition || DEFAULT_TRANSITION;
  const hasOverride = !!scene.transitionIn;

  const handleUpdateSceneName = async () => {
    if (sceneName === scene.name) return;
    try {
      await updateScene(profile.name, scene.id, { name: sceneName });
      updateCurrentScene(scene.id, { name: sceneName });
    } catch (err) {
      toast.error(`Failed to update: ${formatError(err)}`);
      setSceneName(scene.name);
    }
  };

  const handleUpdateSceneTransition = async (updates: Partial<SceneTransition>) => {
    try {
      const newTransition: SceneTransition = {
        ...effectiveTransition,
        ...updates,
      };
      await updateScene(profile.name, scene.id, { transitionIn: newTransition });
      updateCurrentScene(scene.id, { transitionIn: newTransition });
    } catch (err) {
      toast.error(`Failed to update: ${formatError(err)}`);
    }
  };

  const handleUseDefaultTransition = async () => {
    try {
      await updateScene(profile.name, scene.id, { transitionIn: undefined });
      updateCurrentScene(scene.id, { transitionIn: undefined });
    } catch (err) {
      toast.error(`Failed to update: ${formatError(err)}`);
    }
  };

  const handleUpdateProfileDefaultTransition = async (updates: Partial<SceneTransition>) => {
    try {
      const newTransition: SceneTransition = {
        ...(profile.defaultTransition || DEFAULT_TRANSITION),
        ...updates,
      };
      await api.profile.save({ ...profile, defaultTransition: newTransition });
      updateCurrentProfile({ defaultTransition: newTransition });
    } catch (err) {
      toast.error(`Failed to update: ${formatError(err)}`);
    }
  };

  return (
    <div className="space-y-5">
      {/* Scene name */}
      <div>
        <h4 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wide mb-3">
          {t('stream.sceneName', { defaultValue: 'Name' })}
        </h4>
        <Input
          type="text"
          value={sceneName}
          onChange={(e) => setSceneName(e.target.value)}
          onBlur={handleUpdateSceneName}
          onKeyDown={blurOnEnter}
          placeholder={t('stream.sceneNamePlaceholder', { defaultValue: 'Scene name' })}
        />
      </div>

      {/* Canvas dimensions (read-only info) */}
      <div>
        <h4 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wide mb-3">
          {t('stream.canvas', { defaultValue: 'Canvas' })}
        </h4>
        <div className="flex items-center gap-2 px-3 py-2 bg-[var(--bg-sunken)] rounded-lg">
          <Monitor className="w-4 h-4 text-[var(--text-muted)]" />
          <span className="text-sm text-[var(--text-secondary)]">
            {scene.canvasWidth} &times; {scene.canvasHeight}
          </span>
        </div>
      </div>

      {/* Transition settings */}
      <div className="border-t border-[var(--border-default)] pt-4">
        <div className="flex items-center justify-between mb-3">
          <h4 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wide">
            {t('stream.transitionIn', { defaultValue: 'Transition In' })}
          </h4>
          {hasOverride && (
            <button
              type="button"
              onClick={handleUseDefaultTransition}
              className="text-xs text-primary hover:text-primary/80 transition-colors"
            >
              {t('stream.useDefault', { defaultValue: 'Use Default' })}
            </button>
          )}
        </div>

        <div className="space-y-3">
          {/* Transition type selector */}
          <Select
            label={t('stream.transitionType', { defaultValue: 'Type' })}
            value={effectiveTransition.type}
            onChange={(e) => {
              const newType = e.target.value as TransitionType;
              if (hasOverride || !profile.defaultTransition) {
                handleUpdateSceneTransition({ type: newType });
              } else {
                handleUpdateSceneTransition({ type: newType, durationMs: effectiveTransition.durationMs });
              }
            }}
            options={TRANSITION_TYPES.map((type) => ({
              value: type,
              label: getTransitionTypeLabel(type),
            }))}
          />

          {/* Duration slider (hidden for 'cut' which is instant) */}
          {effectiveTransition.type !== 'cut' && (
            <div className="space-y-1">
              <label className="text-xs text-[var(--text-muted)]">
                {t('stream.duration', { defaultValue: 'Duration' })}: {effectiveTransition.durationMs}ms
              </label>
              <input
                type="range"
                min="100"
                max="2000"
                step="50"
                value={effectiveTransition.durationMs}
                onChange={(e) => {
                  const durationMs = Number(e.target.value);
                  if (hasOverride || !profile.defaultTransition) {
                    handleUpdateSceneTransition({ durationMs });
                  } else {
                    handleUpdateSceneTransition({ type: effectiveTransition.type, durationMs });
                  }
                }}
                className="w-full h-2 bg-[var(--bg-sunken)] rounded-lg appearance-none cursor-pointer accent-primary"
              />
              <div className="flex justify-between text-[10px] text-[var(--text-muted)]">
                <span>100ms</span>
                <span>2000ms</span>
              </div>
            </div>
          )}

          {/* Override indicator */}
          {hasOverride && (
            <div className="flex items-center gap-2 px-2 py-1.5 bg-primary/10 border border-primary/20 rounded text-xs text-primary">
              <span>{t('stream.customTransition', { defaultValue: 'Custom transition for this scene' })}</span>
            </div>
          )}
        </div>
      </div>

      {/* Profile default transition section */}
      <div className="border-t border-[var(--border-default)] pt-4">
        <h4 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wide mb-3">
          {t('stream.defaultTransition', { defaultValue: 'Profile Default' })}
        </h4>
        <div className="space-y-3">
          <Select
            label={t('stream.transitionType', { defaultValue: 'Type' })}
            value={(profile.defaultTransition || DEFAULT_TRANSITION).type}
            onChange={(e) => handleUpdateProfileDefaultTransition({ type: e.target.value as TransitionType })}
            options={TRANSITION_TYPES.map((type) => ({
              value: type,
              label: getTransitionTypeLabel(type),
            }))}
          />
          {(profile.defaultTransition || DEFAULT_TRANSITION).type !== 'cut' && (
            <div className="space-y-1">
              <label className="text-xs text-[var(--text-muted)]">
                {t('stream.duration', { defaultValue: 'Duration' })}: {(profile.defaultTransition || DEFAULT_TRANSITION).durationMs}ms
              </label>
              <input
                type="range"
                min="100"
                max="2000"
                step="50"
                value={(profile.defaultTransition || DEFAULT_TRANSITION).durationMs}
                onChange={(e) => handleUpdateProfileDefaultTransition({ durationMs: Number(e.target.value) })}
                className="w-full h-2 bg-[var(--bg-sunken)] rounded-lg appearance-none cursor-pointer accent-primary"
              />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
