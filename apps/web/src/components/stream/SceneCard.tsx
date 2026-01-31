/**
 * Scene Card
 * Mini scene preview card for Multiview panel
 * Uses SceneCanvas in read-only mode (same as Studio Mode Program)
 */
import type { Scene, Profile } from '@/types/profile';
import type { Source } from '@/types/source';
import { cn } from '@/lib/utils';
import { SceneCanvas } from './SceneCanvas';

interface SceneCardProps {
  scene: Scene;
  profile: Profile;
  sources: Source[];
  isPreview?: boolean;
  isProgram?: boolean;
  onClick: () => void;
  size: 'sm' | 'md' | 'lg';
}

export function SceneCard({
  scene,
  profile,
  sources,
  isPreview = false,
  isProgram = false,
  onClick,
  size,
}: SceneCardProps) {
  const sizeClasses = {
    sm: 'h-24',
    md: 'h-36',
    lg: 'h-48',
  };

  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        'relative w-full rounded-lg overflow-hidden border-2 transition-all group',
        sizeClasses[size],
        isProgram && 'border-red-500 shadow-lg shadow-red-500/30',
        isPreview && !isProgram && 'border-green-500 shadow-lg shadow-green-500/30',
        !isPreview && !isProgram && 'border-transparent hover:border-primary/50'
      )}
    >
      {/* Scene preview - uses SceneCanvas in program mode (read-only, same as Studio Mode) */}
      <div className="absolute inset-0 pointer-events-none">
        <SceneCanvas
          scene={scene}
          sources={sources}
          scenes={profile.scenes}
          selectedLayerId={null}
          onSelectLayer={() => {}}
          profileName={profile.name}
          studioMode="program"
        />
      </div>

      {/* Overlay gradient */}
      <div className="absolute inset-x-0 bottom-0 h-12 bg-gradient-to-t from-black/70 to-transparent pointer-events-none" />

      {/* Scene name */}
      <div className="absolute inset-x-0 bottom-0 p-2 flex items-center justify-between">
        <span className="text-xs font-medium text-white truncate">
          {scene.name}
        </span>
        <div className="flex items-center gap-1">
          {isProgram && (
            <span className="px-1.5 py-0.5 text-[10px] font-bold bg-red-600 text-white rounded">
              LIVE
            </span>
          )}
          {isPreview && !isProgram && (
            <span className="px-1.5 py-0.5 text-[10px] font-bold bg-green-600 text-white rounded">
              PVW
            </span>
          )}
        </div>
      </div>

      {/* Hover overlay */}
      <div className="absolute inset-0 bg-primary/10 opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none" />
    </button>
  );
}
