/**
 * Media Playlist Source Editor
 * Extracted from PropertiesPanel — manages playlist items and playback settings
 */
import { useState } from 'react';
import { ListVideo } from 'lucide-react';
import { Select } from '@/components/ui/Select';
import type { MediaPlaylistSource, PlaylistItem, Source } from '@/types/source';
import { PlaylistEditorModal } from '@/components/modals/PlaylistEditorModal';
import type { SourceEditorProps } from './types';

export function MediaPlaylistSourceEditor({
  source,
  profileName,
  updateSource,
  updateCurrentSource,
  t,
}: SourceEditorProps<MediaPlaylistSource>) {
  const [isEditorOpen, setIsEditorOpen] = useState(false);

  const handleSavePlaylist = async (items: PlaylistItem[]) => {
    try {
      const updated = await updateSource(profileName, source.id, { items } as Partial<Source>);
      updateCurrentSource(updated);
      setIsEditorOpen(false);
    } catch (err) {
      console.error('[MediaPlaylistSourceEditor] Failed to save:', err);
    }
  };

  const handleUpdateSetting = async (updates: Partial<MediaPlaylistSource>) => {
    try {
      const updated = await updateSource(profileName, source.id, updates as Partial<Source>);
      updateCurrentSource(updated);
    } catch (err) {
      console.error('[MediaPlaylistSourceEditor] Failed to update:', err);
    }
  };

  return (
    <div className="space-y-4">
      <h4 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wide">
        {t('stream.playlistSettings', { defaultValue: 'Playlist Settings' })}
      </h4>

      {/* Playlist items summary and edit button */}
      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <span className="text-sm text-[var(--text-secondary)]">
            {t('stream.playlistItems', { count: source.items.length, defaultValue: `${source.items.length} items` })}
          </span>
          <button
            type="button"
            onClick={() => setIsEditorOpen(true)}
            className="flex items-center gap-2 px-3 py-1.5 bg-[var(--bg-sunken)] hover:bg-[var(--bg-elevated)] rounded-lg text-sm transition-colors"
          >
            <ListVideo className="w-4 h-4" />
            {t('stream.editPlaylist', { defaultValue: 'Edit Playlist' })}
          </button>
        </div>

        {/* Current item preview */}
        {source.items.length > 0 && (
          <div className="p-2 bg-[var(--bg-sunken)] rounded text-xs">
            <span className="text-[var(--text-muted)]">{t('stream.currentItem', { defaultValue: 'Now Playing:' })}</span>
            <span className="ml-2 text-[var(--text-primary)]">
              {source.items[source.currentItemIndex]?.name || 'Unknown'}
            </span>
          </div>
        )}
      </div>

      {/* Playback settings */}
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <span className="text-sm">{t('stream.autoAdvance', { defaultValue: 'Auto Advance' })}</span>
          <button
            type="button"
            onClick={() => handleUpdateSetting({ autoAdvance: !source.autoAdvance })}
            className={`w-10 h-5 rounded-full transition-colors ${
              source.autoAdvance ? 'bg-primary' : 'bg-[var(--bg-sunken)]'
            }`}
          >
            <div
              className={`w-4 h-4 rounded-full bg-white shadow transition-transform ${
                source.autoAdvance ? 'translate-x-5' : 'translate-x-0.5'
              }`}
            />
          </button>
        </div>

        <div className="flex items-center justify-between">
          <span className="text-sm">{t('stream.fadeBetweenItems', { defaultValue: 'Fade Between Items' })}</span>
          <button
            type="button"
            onClick={() => handleUpdateSetting({ fadeBetweenItems: !source.fadeBetweenItems })}
            className={`w-10 h-5 rounded-full transition-colors ${
              source.fadeBetweenItems ? 'bg-primary' : 'bg-[var(--bg-sunken)]'
            }`}
          >
            <div
              className={`w-4 h-4 rounded-full bg-white shadow transition-transform ${
                source.fadeBetweenItems ? 'translate-x-5' : 'translate-x-0.5'
              }`}
            />
          </button>
        </div>

        <Select
          label={t('stream.shuffleMode', { defaultValue: 'Shuffle Mode' })}
          value={source.shuffleMode}
          onChange={(e) => handleUpdateSetting({ shuffleMode: e.target.value as 'none' | 'all' | 'repeat-one' })}
          options={[
            { value: 'none', label: t('stream.shuffleNone', { defaultValue: 'None' }) },
            { value: 'all', label: t('stream.shuffleAll', { defaultValue: 'Shuffle All' }) },
            { value: 'repeat-one', label: t('stream.repeatOne', { defaultValue: 'Repeat One' }) },
          ]}
        />
      </div>

      {/* Playlist editor modal */}
      <PlaylistEditorModal
        open={isEditorOpen}
        onClose={() => setIsEditorOpen(false)}
        source={source}
        onSave={handleSavePlaylist}
      />
    </div>
  );
}
