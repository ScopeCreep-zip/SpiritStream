/**
 * useSceneActions — Scene CRUD callbacks extracted from SceneBar
 *
 * Provides stable memoized handlers for scene create/delete/duplicate/select,
 * projector opening, and multiview. Keeps SceneBar as a thin render layer.
 */
import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import type { Scene } from '@/types/scene';
import { useSceneStore } from '@/stores/sceneStore';
import { useProfileStore } from '@/stores/profileStore';
import { useStudioStore } from '@/stores/studioStore';
import { useTransitionStore } from '@/stores/transitionStore';
import { useProjectorStore } from '@/stores/projectorStore';
import { toast } from '@/hooks/useToast';
import { useShallow } from 'zustand/shallow';

interface UseSceneActionsParams {
  profileName: string;
  activeSceneId?: string;
  sceneCount: number;
}

export function useSceneActions({ profileName, activeSceneId, sceneCount }: UseSceneActionsParams) {
  const { t } = useTranslation();

  const { createScene, deleteScene, duplicateScene, setActiveScene } = useSceneStore(
    useShallow(s => ({
      createScene: s.createScene,
      deleteScene: s.deleteScene,
      duplicateScene: s.duplicateScene,
      setActiveScene: s.setActiveScene,
    }))
  );

  const { addCurrentScene, removeCurrentScene, setCurrentActiveScene, reloadProfile } = useProfileStore(
    useShallow(s => ({
      addCurrentScene: s.addCurrentScene,
      removeCurrentScene: s.removeCurrentScene,
      setCurrentActiveScene: s.setCurrentActiveScene,
      reloadProfile: s.reloadProfile,
    }))
  );

  const { enabled: studioEnabled, previewSceneId, programSceneId, setPreviewScene } = useStudioStore(
    useShallow(s => ({
      enabled: s.enabled,
      previewSceneId: s.previewSceneId,
      programSceneId: s.programSceneId,
      setPreviewScene: s.setPreviewScene,
    }))
  );

  const { isTransitioning } = useTransitionStore();

  const { openProjector, hasActiveProjectors } = useProjectorStore(
    useShallow(s => ({
      openProjector: s.openProjector,
      hasActiveProjectors: s.hasActiveProjectors,
    }))
  );

  const handleCreateScene = useCallback(async (
    sceneName: string,
    onSuccess: () => void,
  ) => {
    if (!sceneName.trim()) {
      toast.error(t('stream.sceneNameRequired', { defaultValue: 'Scene name is required' }));
      return;
    }

    try {
      const createdScene = await createScene(profileName, sceneName.trim());
      onSuccess();

      if (createdScene && typeof createdScene === 'object' && 'id' in createdScene) {
        addCurrentScene(createdScene as Scene);
      } else {
        await reloadProfile();
      }
      toast.success(t('stream.sceneCreated', { defaultValue: 'Scene created' }));
    } catch (err) {
      toast.error(t('stream.sceneCreateFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to create scene: ${err instanceof Error ? err.message : String(err)}` }));
    }
  }, [profileName, createScene, addCurrentScene, reloadProfile, t]);

  const handleDeleteScene = useCallback(async (sceneId: string, sceneName: string) => {
    if (sceneCount <= 1) {
      toast.error(t('stream.cannotDeleteLastScene', { defaultValue: 'Cannot delete the last scene' }));
      return;
    }

    if (confirm(t('stream.confirmDeleteScene', { name: sceneName, defaultValue: `Delete scene "${sceneName}"?` }))) {
      try {
        await deleteScene(profileName, sceneId);
        removeCurrentScene(sceneId);
        toast.success(t('stream.sceneDeleted', { defaultValue: 'Scene deleted' }));
      } catch (err) {
        toast.error(t('stream.sceneDeleteFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to delete scene: ${err instanceof Error ? err.message : String(err)}` }));
      }
    }
  }, [profileName, sceneCount, deleteScene, removeCurrentScene, t]);

  const handleDuplicateScene = useCallback(async (sceneId: string) => {
    try {
      const duplicatedScene = await duplicateScene(profileName, sceneId);

      if (duplicatedScene && typeof duplicatedScene === 'object' && 'id' in duplicatedScene) {
        addCurrentScene(duplicatedScene as Scene);
      } else {
        await reloadProfile();
      }
      toast.success(t('stream.sceneDuplicated', { defaultValue: 'Scene duplicated' }));
    } catch (err) {
      toast.error(t('stream.sceneDuplicateFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to duplicate scene: ${err instanceof Error ? err.message : String(err)}` }));
    }
  }, [profileName, duplicateScene, addCurrentScene, reloadProfile, t]);

  const handleSelectScene = useCallback(async (sceneId: string) => {
    if (isTransitioning) return;

    if (studioEnabled) {
      if (sceneId !== previewSceneId) {
        setPreviewScene(sceneId);
      }
      return;
    }

    if (sceneId === activeSceneId) return;

    try {
      await setActiveScene(profileName, sceneId);
      setCurrentActiveScene(sceneId);
    } catch (err) {
      toast.error(t('stream.sceneSwitchFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to switch scene: ${err instanceof Error ? err.message : String(err)}` }));
    }
  }, [activeSceneId, profileName, setActiveScene, setCurrentActiveScene, t, studioEnabled, previewSceneId, setPreviewScene, isTransitioning]);

  const handleOpenProjector = useCallback(() => {
    const sceneToProject = studioEnabled ? programSceneId : activeSceneId;
    if (sceneToProject) {
      openProjector({
        type: 'scene',
        displayMode: 'windowed',
        targetId: sceneToProject,
        profileName,
        alwaysOnTop: false,
        hideCursor: false,
      });
    } else {
      toast.error(t('stream.noSceneToProject', { defaultValue: 'No scene to project' }));
    }
  }, [studioEnabled, programSceneId, activeSceneId, profileName, openProjector, t]);

  const handleOpenMultiview = useCallback(() => {
    openProjector({
      type: 'multiview',
      displayMode: 'windowed',
      profileName,
      alwaysOnTop: false,
      hideCursor: false,
    });
  }, [profileName, openProjector]);

  return {
    handleCreateScene,
    handleDeleteScene,
    handleDuplicateScene,
    handleSelectScene,
    handleOpenProjector,
    handleOpenMultiview,
    studioEnabled,
    previewSceneId,
    programSceneId,
    isTransitioning,
    hasActiveProjectors,
  };
}
