/**
 * Scene Bar
 * Horizontal bar with scene tabs for quick switching
 *
 * Optimized to use local state updates instead of reloading the entire profile
 * after each scene operation, preventing WebRTC reconnections and improving UX.
 */
import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Copy, Trash2 } from 'lucide-react';
import { Button } from '@/components/ui/Button';
import type { Profile } from '@/types/profile';
import type { Scene } from '@/types/scene';
import { useSceneStore } from '@/stores/sceneStore';
import { useProfileStore } from '@/stores/profileStore';
import { toast } from '@/hooks/useToast';

interface SceneBarProps {
  profile: Profile;
  activeSceneId?: string;
}

export function SceneBar({ profile, activeSceneId }: SceneBarProps) {
  const { t } = useTranslation();
  const { createScene, deleteScene, duplicateScene, setActiveScene } = useSceneStore();
  const { addCurrentScene, removeCurrentScene, setCurrentActiveScene, reloadProfile } = useProfileStore();
  const [showNewSceneInput, setShowNewSceneInput] = useState(false);
  const [newSceneName, setNewSceneName] = useState('');

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
    if (sceneId === activeSceneId) return;

    try {
      await setActiveScene(profile.name, sceneId);
      // Update local state instead of reloading entire profile
      setCurrentActiveScene(sceneId);
    } catch (err) {
      toast.error(t('stream.sceneSwitchFailed', { error: err instanceof Error ? err.message : String(err), defaultValue: `Failed to switch scene: ${err instanceof Error ? err.message : String(err)}` }));
    }
  }, [activeSceneId, profile.name, setActiveScene, setCurrentActiveScene, t]);

  return (
    <div className="flex items-center gap-2 px-4 py-3 bg-card rounded border border-border overflow-x-auto">
      <span className="text-xs font-medium text-muted">{t('stream.scenes', { defaultValue: 'Scenes' })}</span>

      {/* Scene tabs */}
      {profile.scenes.map((scene) => (
        <div
          key={scene.id}
          className={`group flex items-center gap-2 px-4 py-2 rounded cursor-pointer transition-colors min-h-[40px] ${
            scene.id === activeSceneId
              ? 'bg-primary text-primary-foreground'
              : 'bg-muted/30 hover:bg-muted/50'
          }`}
          onClick={() => handleSelectScene(scene.id)}
        >
          <span className="text-sm font-medium">{scene.name}</span>
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
      ))}

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
    </div>
  );
}
