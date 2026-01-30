/**
 * useHotkeys Hook
 * Handles global keyboard shortcuts for the Stream view
 */
import { useEffect } from 'react';
import { useHotkeyStore } from '@/stores/hotkeyStore';
import { useStudioStore } from '@/stores/studioStore';
import { useProfileStore } from '@/stores/profileStore';
import { useSceneStore } from '@/stores/sceneStore';

export function useHotkeys() {
  const { enabled, bindings } = useHotkeyStore();
  const {
    executeTake,
    setPreviewScene,
    toggleStudioMode,
    enabled: studioEnabled,
  } = useStudioStore();
  const current = useProfileStore((s) => s.current);
  const { selectLayer } = useSceneStore();

  useEffect(() => {
    if (!enabled) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      // Skip if typing in an input, textarea, or contenteditable element
      const target = e.target as HTMLElement;
      if (
        target.tagName === 'INPUT' ||
        target.tagName === 'TEXTAREA' ||
        target.isContentEditable
      ) {
        return;
      }

      // Find matching binding
      const binding = bindings.find((b) => {
        if (!b.enabled || b.key !== e.code) return false;
        if (!!b.modifiers.ctrl !== e.ctrlKey) return false;
        if (!!b.modifiers.alt !== e.altKey) return false;
        if (!!b.modifiers.shift !== e.shiftKey) return false;
        if (!!b.modifiers.meta !== e.metaKey) return false;
        return true;
      });

      if (!binding) return;

      // Prevent default for matched bindings
      e.preventDefault();
      e.stopPropagation();

      // Execute the action
      switch (binding.action) {
        case 'take':
          // Only execute take in studio mode
          if (studioEnabled) {
            executeTake();
          }
          break;

        case 'escape':
          // Deselect current layer
          selectLayer(null);
          break;

        case 'toggleStudioMode':
          toggleStudioMode();
          break;

        default:
          // Handle scene switching (scene1-scene9)
          if (binding.action.startsWith('scene')) {
            const index = parseInt(binding.action.replace('scene', '')) - 1;
            const scene = current?.scenes[index];
            if (scene) {
              if (studioEnabled) {
                // In studio mode, load to preview
                setPreviewScene(scene.id);
              } else {
                // In normal mode, switch active scene
                // This is handled by the SceneBar, so we just load to preview
                // which will sync when not in studio mode
                setPreviewScene(scene.id);
              }
            }
          }
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [
    enabled,
    bindings,
    studioEnabled,
    current,
    executeTake,
    setPreviewScene,
    selectLayer,
    toggleStudioMode,
  ]);
}
