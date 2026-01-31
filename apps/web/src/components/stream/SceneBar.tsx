/**
 * Scene Bar
 * Horizontal bar with scene tabs for quick switching
 *
 * Optimized to use local state updates instead of reloading the entire profile
 * after each scene operation, preventing WebRTC reconnections and improving UX.
 *
 * Features:
 * - Scene tabs with right-click projector context menu
 * - Quick projector button for current scene
 * - Studio mode tally indicators
 */
import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Copy, Trash2, MonitorPlay, LayoutGrid } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { ProjectorContextMenu, ContextMenuSeparator, ContextMenuItem, useContextMenu } from '@/components/ui/ProjectorContextMenu';
import type { Profile } from '@/types/profile';
import type { Scene } from '@/types/scene';
import { useSceneStore } from '@/stores/sceneStore';
import { useProfileStore } from '@/stores/profileStore';
import { useStudioStore } from '@/stores/studioStore';
import { useTransitionStore } from '@/stores/transitionStore';
import { useProjectorStore } from '@/stores/projectorStore';
import { toast } from '@/hooks/useToast';
import { cn } from '@/lib/utils';

interface SceneBarProps {
  profile: Profile;
  activeSceneId?: string;
}

export function SceneBar({ profile, activeSceneId }: SceneBarProps) {
  const { t } = useTranslation();
  const { createScene, deleteScene, duplicateScene, setActiveScene } = useSceneStore();
  const { addCurrentScene, removeCurrentScene, setCurrentActiveScene, reloadProfile } = useProfileStore();
  const { enabled: studioEnabled, previewSceneId, programSceneId, setPreviewScene } = useStudioStore();
  const { isTransitioning } = useTransitionStore();
  const { openProjector, hasActiveProjectors } = useProjectorStore();
  const [showNewSceneInput, setShowNewSceneInput] = useState(false);
  const [newSceneName, setNewSceneName] = useState('');

  // Context menu state for scene tabs
  const [contextMenuScene, setContextMenuScene] = useState<Scene | null>(null);
  const sceneContextMenu = useContextMenu();

  const handleCreateScene = useCallback(async () => {
    if (!newSceneName.trim()) {
      toast.error(t('stream.sceneNameRequired', { defaultValue: 'Scene name is required' }));
      return;
    }

    try {
      // Create scene on backend - returns the created scene
      const createdScene = await createScene(profile.name, newSceneName.trim());
      setNewSceneName('');
      setShowNewSceneInput(false);

      // If the API returns the created scene, use it for local update
      if (createdScene && typeof createdScene === 'object' && 'id' in createdScene) {
        addCurrentScene(createdScene as Scene);
      } else {
        // Fallback: reload profile if API doesn't return scene data
        await reloadProfile();
      }
      toast.success(t('stream.sceneCreated', { defaultValue: 'Scene created' }));
    } catch (err) {
      toast.error(t('stream.sceneCreateFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to create scene: ${err instanceof Error ? err.message : String(err)}` }));
    }
  }, [newSceneName, profile.name, createScene, addCurrentScene, reloadProfile, t]);

  const handleDeleteScene = useCallback(async (sceneId: string, sceneName: string) => {
    if (profile.scenes.length <= 1) {
      toast.error(t('stream.cannotDeleteLastScene', { defaultValue: 'Cannot delete the last scene' }));
      return;
    }

    if (confirm(t('stream.confirmDeleteScene', { name: sceneName, defaultValue: `Delete scene "${sceneName}"?` }))) {
      try {
        await deleteScene(profile.name, sceneId);
        // Update local state instead of reloading entire profile
        removeCurrentScene(sceneId);
        toast.success(t('stream.sceneDeleted', { defaultValue: 'Scene deleted' }));
      } catch (err) {
        toast.error(t('stream.sceneDeleteFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to delete scene: ${err instanceof Error ? err.message : String(err)}` }));
      }
    }
  }, [profile.name, profile.scenes.length, deleteScene, removeCurrentScene, t]);

  const handleDuplicateScene = useCallback(async (sceneId: string) => {
    try {
      // Duplicate scene on backend - returns the duplicated scene
      const duplicatedScene = await duplicateScene(profile.name, sceneId);

      // If the API returns the duplicated scene, use it for local update
      if (duplicatedScene && typeof duplicatedScene === 'object' && 'id' in duplicatedScene) {
        addCurrentScene(duplicatedScene as Scene);
      } else {
        // Fallback: reload profile if API doesn't return scene data
        await reloadProfile();
      }
      toast.success(t('stream.sceneDuplicated', { defaultValue: 'Scene duplicated' }));
    } catch (err) {
      toast.error(t('stream.sceneDuplicateFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to duplicate scene: ${err instanceof Error ? err.message : String(err)}` }));
    }
  }, [profile.name, duplicateScene, addCurrentScene, reloadProfile, t]);

  const handleSelectScene = useCallback(async (sceneId: string) => {
    // Prevent clicks during transition
    if (isTransitioning) return;

    // In Studio Mode, clicking loads to Preview only
    if (studioEnabled) {
      if (sceneId !== previewSceneId) {
        setPreviewScene(sceneId);
      }
      return;
    }

    // Normal mode: switch active scene
    if (sceneId === activeSceneId) return;

    try {
      await setActiveScene(profile.name, sceneId);
      // Update local state instead of reloading entire profile
      setCurrentActiveScene(sceneId);
    } catch (err) {
      toast.error(t('stream.sceneSwitchFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to switch scene: ${err instanceof Error ? err.message : String(err)}` }));
    }
  }, [activeSceneId, profile.name, setActiveScene, setCurrentActiveScene, t, studioEnabled, previewSceneId, setPreviewScene, isTransitioning]);

  const handleOpenProjector = useCallback(() => {
    // In Studio Mode, project the Program scene; in Normal Mode, project the active scene
    const sceneToProject = studioEnabled ? programSceneId : activeSceneId;
    if (sceneToProject) {
      openProjector({
        type: 'scene',
        displayMode: 'windowed',
        targetId: sceneToProject,
        profileName: profile.name,
        alwaysOnTop: false,
        hideCursor: false,
      });
    } else {
      toast.error(t('stream.noSceneToProject', { defaultValue: 'No scene to project' }));
    }
  }, [studioEnabled, programSceneId, activeSceneId, profile.name, openProjector, t]);

  const handleOpenMultiview = useCallback(() => {
    openProjector({
      type: 'multiview',
      displayMode: 'windowed',
      profileName: profile.name,
      alwaysOnTop: false,
      hideCursor: false,
    });
  }, [profile.name, openProjector]);

  const handleSceneContextMenu = useCallback((e: React.MouseEvent, scene: Scene) => {
    e.preventDefault();
    e.stopPropagation();
    setContextMenuScene(scene);
    sceneContextMenu.open(e);
  }, [sceneContextMenu]);

  return (
    <div className="flex items-center gap-2 px-4 py-3 bg-card rounded border border-border overflow-x-auto">
      <span className="text-xs font-medium text-muted">{t('stream.scenes', { defaultValue: 'Scenes' })}</span>

      {/* Scene tabs */}
      {profile.scenes.map((scene) => {
        const isActive = scene.id === activeSceneId;
        const isPreview = studioEnabled && scene.id === previewSceneId;
        const isProgram = studioEnabled && scene.id === programSceneId;
        const hasTransitionOverride = !!scene.transitionIn;

        return (
          <div
            key={scene.id}
            className={cn(
              'group relative flex items-center gap-2 px-4 py-2 rounded cursor-pointer transition-all min-h-[40px]',
              // Normal mode styling
              !studioEnabled && isActive && 'bg-primary text-primary-foreground',
              !studioEnabled && !isActive && 'bg-muted/30 hover:bg-muted/50',
              // Studio mode styling
              studioEnabled && isPreview && 'ring-2 ring-green-500 bg-green-500/10',
              studioEnabled && isProgram && 'ring-2 ring-red-500 bg-red-500/10',
              studioEnabled && !isPreview && !isProgram && 'bg-muted/30 hover:bg-muted/50',
              // Transition in progress
              isTransitioning && 'opacity-50 cursor-not-allowed'
            )}
            onClick={() => handleSelectScene(scene.id)}
            onContextMenu={(e) => handleSceneContextMenu(e, scene)}
          >
            {/* Transition override indicator */}
            {hasTransitionOverride && (
              <span className="absolute -top-1 -right-1 w-2 h-2 bg-[var(--accent)] rounded-full" />
            )}

            <span className="text-sm font-medium">{scene.name}</span>

            {/* Live indicator in Studio Mode */}
            {isProgram && (
              <span className="text-[10px] font-bold text-red-500 flex items-center gap-1">
                <span className="w-1.5 h-1.5 bg-red-500 rounded-full animate-pulse" />
                LIVE
              </span>
            )}

            <div className="flex items-center gap-1 opacity-40 group-hover:opacity-100 transition-opacity">
              <button
                className="p-1 hover:bg-white/20 rounded min-w-[24px] min-h-[24px] flex items-center justify-center"
                onClick={(e) => {
                  e.stopPropagation();
                  handleDuplicateScene(scene.id);
                }}
                title={t('common.duplicate')}
              >
                <Copy className="w-3.5 h-3.5" />
              </button>
              {profile.scenes.length > 1 && (
                <button
                  className="p-1 hover:bg-destructive/50 rounded min-w-[24px] min-h-[24px] flex items-center justify-center"
                  onClick={(e) => {
                    e.stopPropagation();
                    handleDeleteScene(scene.id, scene.name);
                  }}
                  title={t('common.delete')}
                >
                  <Trash2 className="w-3.5 h-3.5" />
                </button>
              )}
            </div>
          </div>
        );
      })}

      {/* New scene input */}
      {showNewSceneInput ? (
        <div className="flex items-stretch gap-2">
          <input
            type="text"
            value={newSceneName}
            onChange={(e) => setNewSceneName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') handleCreateScene();
              if (e.key === 'Escape') {
                setShowNewSceneInput(false);
                setNewSceneName('');
              }
            }}
            placeholder={t('stream.sceneName', { defaultValue: 'Scene name' })}
            className="w-36 px-3 py-2 text-sm bg-background border border-border rounded focus:outline-none focus:ring-2 focus:ring-primary/50 focus:border-primary"
            autoFocus
          />
          <Button size="sm" variant="primary" className="h-auto" onClick={handleCreateScene}>
            {t('common.add')}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            className="h-auto"
            onClick={() => {
              setShowNewSceneInput(false);
              setNewSceneName('');
            }}
          >
            {t('common.cancel')}
          </Button>
        </div>
      ) : (
        <Button
          variant="ghost"
          size="sm"
          className="min-w-[36px] min-h-[36px]"
          onClick={() => setShowNewSceneInput(true)}
          title={t('stream.addScene', { defaultValue: 'Add Scene' })}
        >
          <Plus className="w-4 h-4" />
        </Button>
      )}

      {/* Divider */}
      <div className="h-6 w-px bg-border mx-2" />

      {/* Multiview button */}
      <Button
        variant="ghost"
        size="sm"
        className="min-w-[36px] min-h-[36px]"
        onClick={handleOpenMultiview}
        title={t('stream.multiviewProjector', { defaultValue: 'Open Multiview Projector' })}
      >
        <LayoutGrid className="w-4 h-4" />
      </Button>

      {/* Projector button */}
      <Button
        variant={hasActiveProjectors() ? 'primary' : 'ghost'}
        size="sm"
        className="min-w-[36px] min-h-[36px]"
        onClick={handleOpenProjector}
        title={t('stream.projector', { defaultValue: 'Open Projector (Fullscreen)' })}
      >
        <MonitorPlay className="w-4 h-4" />
      </Button>

      {/* Scene context menu */}
      {sceneContextMenu.isOpen && contextMenuScene && (
        <ProjectorContextMenu
          type="scene"
          targetId={contextMenuScene.id}
          profileName={profile.name}
          position={sceneContextMenu.position}
          onClose={() => {
            sceneContextMenu.close();
            setContextMenuScene(null);
          }}
          typeLabel={contextMenuScene.name}
          additionalItemsAfter={
            <>
              <ContextMenuSeparator />
              <ContextMenuItem
                icon={<Copy className="w-4 h-4" />}
                label={t('common.duplicate', { defaultValue: 'Duplicate Scene' })}
                onClick={() => {
                  handleDuplicateScene(contextMenuScene.id);
                  sceneContextMenu.close();
                  setContextMenuScene(null);
                }}
              />
              {profile.scenes.length > 1 && (
                <ContextMenuItem
                  icon={<Trash2 className="w-4 h-4" />}
                  label={t('common.delete', { defaultValue: 'Delete Scene' })}
                  onClick={() => {
                    handleDeleteScene(contextMenuScene.id, contextMenuScene.name);
                    sceneContextMenu.close();
                    setContextMenuScene(null);
                  }}
                  destructive
                />
              )}
            </>
          }
        />
      )}
    </div>
  );
}
