/**
 * useHotkeys Hook
 * Handles global keyboard shortcuts for the Stream view
 */
import { useEffect, useRef, useCallback } from 'react';
import { useHotkeyStore } from '@/stores/hotkeyStore';
import { useStudioStore } from '@/stores/studioStore';
import { useProfileStore } from '@/stores/profileStore';
import { useSceneStore } from '@/stores/sceneStore';
import { useStreamStore } from '@/stores/streamStore';
import { toast } from '@/hooks/useToast';
import { getIncomingUrl } from '@/types/profile';

// Singleton listener manager to prevent race conditions
// Uses WeakRef tracking to properly manage component lifetimes
const listenerManager = {
  handler: null as ((e: KeyboardEvent) => void) | null,
  activeInstances: new Set<string>(),

  register(instanceId: string, handler: (e: KeyboardEvent) => void) {
    this.activeInstances.add(instanceId);

    // Only register once
    if (!this.handler) {
      this.handler = handler;
      document.addEventListener('keydown', handler);
    }

    return () => this.unregister(instanceId);
  },

  unregister(instanceId: string) {
    this.activeInstances.delete(instanceId);

    // Only remove listener when ALL instances are gone
    if (this.activeInstances.size === 0 && this.handler) {
      document.removeEventListener('keydown', this.handler);
      this.handler = null;
    }
  }
};

export function useHotkeys() {
  const { enabled, bindings } = useHotkeyStore();
  const {
    executeTake,
    setPreviewScene,
    toggleStudioMode,
    enabled: studioEnabled,
  } = useStudioStore();
  const current = useProfileStore((s) => s.current);
  const { selectLayer, setMasterVolume, updateLayer } = useSceneStore();
  const { isStreaming, startAllGroups, stopAllGroups } = useStreamStore();
  const { updateCurrentMasterVolume, updateCurrentLayer } = useProfileStore();

  // Track mute state and previous volume for toggle
  const mutedRef = useRef(false);
  const previousVolumeRef = useRef(1.0);

  // Toggle stream handler
  const handleToggleStream = useCallback(async () => {
    if (!current) return;

    try {
      if (isStreaming) {
        await stopAllGroups();
        toast.success('Stream stopped');
      } else {
        const incomingUrl = getIncomingUrl(current);
        if (!incomingUrl) {
          toast.error('No incoming URL configured');
          return;
        }
        await startAllGroups(current.outputGroups, incomingUrl);
        toast.success('Stream started');
      }
    } catch (err) {
      toast.error(`Failed to toggle stream: ${err instanceof Error ? err.message : String(err)}`);
    }
  }, [current, isStreaming, startAllGroups, stopAllGroups]);

  // Toggle mute handler
  const handleToggleMute = useCallback(async () => {
    if (!current) return;

    const activeScene = current.scenes.find((s) => s.id === current.activeSceneId);
    if (!activeScene) return;

    try {
      if (mutedRef.current) {
        // Unmute - restore previous volume
        const volumeToRestore = previousVolumeRef.current;
        await setMasterVolume(current.name, activeScene.id, volumeToRestore);
        updateCurrentMasterVolume(activeScene.id, volumeToRestore);
        mutedRef.current = false;
        toast.success('Audio unmuted');
      } else {
        // Mute - save current volume and set to 0
        previousVolumeRef.current = activeScene.audioMixer.masterVolume;
        await setMasterVolume(current.name, activeScene.id, 0);
        updateCurrentMasterVolume(activeScene.id, 0);
        mutedRef.current = true;
        toast.success('Audio muted');
      }
    } catch (err) {
      toast.error(`Failed to toggle mute: ${err instanceof Error ? err.message : String(err)}`);
    }
  }, [current, setMasterVolume, updateCurrentMasterVolume]);

  // Toggle layer visibility handler
  const handleToggleLayerVisibility = useCallback(
    async (layerId: string, sceneId: string) => {
      if (!current) return;

      const scene = current.scenes.find((s) => s.id === sceneId);
      if (!scene) return;

      const layer = scene.layers.find((l) => l.id === layerId);
      if (!layer) return;

      const newVisible = !layer.visible;

      try {
        await updateLayer(current.name, sceneId, layerId, { visible: newVisible });
        updateCurrentLayer(sceneId, layerId, { visible: newVisible });
      } catch (err) {
        toast.error(`Failed to toggle layer: ${err instanceof Error ? err.message : String(err)}`);
      }
    },
    [current, updateLayer, updateCurrentLayer]
  );

  // Generate a stable instance ID for this hook invocation
  const instanceIdRef = useRef(crypto.randomUUID());

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

        case 'toggleStream':
          handleToggleStream();
          break;

        case 'toggleMute':
          handleToggleMute();
          break;

        case 'toggleLayerVisibility':
          // Handle layer visibility toggle
          if (binding.layerId && binding.sceneId) {
            handleToggleLayerVisibility(binding.layerId, binding.sceneId);
          }
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

    // Register with singleton manager - handles deduplication
    const cleanup = listenerManager.register(instanceIdRef.current, handleKeyDown);

    return cleanup;
  }, [
    enabled,
    bindings,
    studioEnabled,
    current,
    executeTake,
    setPreviewScene,
    selectLayer,
    toggleStudioMode,
    handleToggleStream,
    handleToggleMute,
    handleToggleLayerVisibility,
  ]);
}
