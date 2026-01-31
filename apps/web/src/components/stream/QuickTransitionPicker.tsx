/**
 * Quick Transition Picker
 * Compact dropdown for selecting transition type in Studio Mode
 */
import { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Film, Image, VolumeX, Volume2 } from 'lucide-react';
import { useProfileStore } from '@/stores/profileStore';
import { dialogs } from '@/lib/backend/dialogs';
import { backendMode } from '@/lib/backend/env';
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
  const [isSelectingFile, setIsSelectingFile] = useState(false);

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

  // Handle stinger file selection
  const handleSelectStingerFile = useCallback(async () => {
    setIsSelectingFile(true);
    try {
      if (backendMode === 'tauri') {
        const filePath = await dialogs.openFilePath();
        if (filePath) {
          await updateProfile({
            defaultTransition: {
              ...currentTransition,
              stingerFilePath: filePath,
            },
          });
        }
      } else {
        // HTTP mode: use file input with object URL
        const input = document.createElement('input');
        input.type = 'file';
        input.accept = 'video/*,.mp4,.webm,.mov';
        input.onchange = async () => {
          if (input.files && input.files[0]) {
            const objectUrl = URL.createObjectURL(input.files[0]);
            await updateProfile({
              defaultTransition: {
                ...currentTransition,
                stingerFilePath: objectUrl,
              },
            });
          }
        };
        input.click();
      }
    } finally {
      setIsSelectingFile(false);
    }
  }, [currentTransition, updateProfile]);

  // Handle luma wipe image selection
  const handleSelectLumaImage = useCallback(async () => {
    setIsSelectingFile(true);
    try {
      if (backendMode === 'tauri') {
        const filePath = await dialogs.openFilePath();
        if (filePath) {
          await updateProfile({
            defaultTransition: {
              ...currentTransition,
              lumaWipeImage: filePath,
            },
          });
        }
      } else {
        // HTTP mode: use file input with object URL
        const input = document.createElement('input');
        input.type = 'file';
        input.accept = 'image/*,.png,.jpg,.jpeg';
        input.onchange = async () => {
          if (input.files && input.files[0]) {
            const objectUrl = URL.createObjectURL(input.files[0]);
            await updateProfile({
              defaultTransition: {
                ...currentTransition,
                lumaWipeImage: objectUrl,
              },
            });
          }
        };
        input.click();
      }
    } finally {
      setIsSelectingFile(false);
    }
  }, [currentTransition, updateProfile]);

  // Toggle stinger audio mute
  const handleToggleStingerAudio = useCallback(async () => {
    await updateProfile({
      defaultTransition: {
        ...currentTransition,
        stingerAudioMuted: !currentTransition.stingerAudioMuted,
      },
    });
  }, [currentTransition, updateProfile]);

  // Toggle luma wipe invert
  const handleToggleLumaInvert = useCallback(async () => {
    await updateProfile({
      defaultTransition: {
        ...currentTransition,
        lumaWipeInvert: !currentTransition.lumaWipeInvert,
      },
    });
  }, [currentTransition, updateProfile]);

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

      {/* Fade to Color controls */}
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

      {/* Stinger controls */}
      {currentTransition.type === 'stinger' && (
        <div className="flex flex-col items-center gap-1">
          <button
            onClick={handleSelectStingerFile}
            disabled={isSelectingFile}
            className="flex items-center gap-1 px-2 py-0.5 text-[9px] bg-[var(--bg-sunken)] hover:bg-[var(--bg-hover)] border border-[var(--border-default)] rounded text-[var(--text-secondary)] disabled:opacity-50"
            title={t('stream.selectStingerVideo', { defaultValue: 'Select Stinger Video' })}
          >
            <Film className="w-3 h-3" />
            {currentTransition.stingerFilePath
              ? currentTransition.stingerFilePath.split('/').pop()?.slice(0, 8) + '...'
              : t('stream.selectVideo', { defaultValue: 'Select...' })}
          </button>
          <button
            onClick={handleToggleStingerAudio}
            className={`p-1 rounded ${
              currentTransition.stingerAudioMuted
                ? 'bg-[var(--bg-sunken)] text-[var(--text-muted)]'
                : 'bg-primary/20 text-primary'
            }`}
            title={
              currentTransition.stingerAudioMuted
                ? t('stream.enableStingerAudio', { defaultValue: 'Enable Audio' })
                : t('stream.muteStingerAudio', { defaultValue: 'Mute Audio' })
            }
          >
            {currentTransition.stingerAudioMuted ? (
              <VolumeX className="w-3 h-3" />
            ) : (
              <Volume2 className="w-3 h-3" />
            )}
          </button>
        </div>
      )}

      {/* Luma Wipe controls */}
      {currentTransition.type === 'lumaWipe' && (
        <div className="flex flex-col items-center gap-1">
          <button
            onClick={handleSelectLumaImage}
            disabled={isSelectingFile}
            className="flex items-center gap-1 px-2 py-0.5 text-[9px] bg-[var(--bg-sunken)] hover:bg-[var(--bg-hover)] border border-[var(--border-default)] rounded text-[var(--text-secondary)] disabled:opacity-50"
            title={t('stream.selectLumaImage', { defaultValue: 'Select Luma Mask Image' })}
          >
            <Image className="w-3 h-3" />
            {currentTransition.lumaWipeImage
              ? currentTransition.lumaWipeImage.split('/').pop()?.slice(0, 8) + '...'
              : t('stream.selectImage', { defaultValue: 'Select...' })}
          </button>
          <label className="flex items-center gap-1 text-[9px] text-[var(--text-muted)] cursor-pointer">
            <input
              type="checkbox"
              checked={currentTransition.lumaWipeInvert ?? false}
              onChange={handleToggleLumaInvert}
              className="w-3 h-3"
            />
            {t('stream.invertLuma', { defaultValue: 'Invert' })}
          </label>
        </div>
      )}
    </div>
  );
}
