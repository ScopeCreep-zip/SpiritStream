import React from 'react';
import { useTranslation } from 'react-i18next';
import { Input } from '@/components/ui/Input';
import { Select } from '@/components/ui/Select';
import { Toggle } from '@/components/ui/Toggle';
import type { MediaPlaylistSource } from '@/types/source';

interface MediaPlaylistFormProps {
  data: MediaPlaylistSource;
  onChange: (data: MediaPlaylistSource) => void;
}

export const MediaPlaylistForm = React.memo(({ data, onChange }: MediaPlaylistFormProps) => {
  const { t } = useTranslation();

  return (
    <div className="flex flex-col gap-4">
      <Input
        label={t('stream.sourceName', { defaultValue: 'Source Name' })}
        value={data.name}
        onChange={(e) => onChange({ ...data, name: e.target.value })}
        placeholder="Media Playlist"
      />
      <p className="text-sm text-muted">
        {t('stream.playlistHelper', { defaultValue: 'Add media files to the playlist after creating the source.' })}
      </p>
      <div className="flex items-center justify-between">
        <span className="text-sm">{t('stream.autoAdvance', { defaultValue: 'Auto Advance' })}</span>
        <Toggle
          checked={data.autoAdvance}
          onChange={(checked) => onChange({ ...data, autoAdvance: checked })}
        />
      </div>
      <div className="flex items-center justify-between">
        <span className="text-sm">{t('stream.fadeBetweenItems', { defaultValue: 'Fade Between Items' })}</span>
        <Toggle
          checked={data.fadeBetweenItems}
          onChange={(checked) => onChange({ ...data, fadeBetweenItems: checked })}
        />
      </div>
      {data.fadeBetweenItems && (
        <Input
          label={t('stream.fadeDuration', { defaultValue: 'Fade Duration (ms)' })}
          type="number"
          value={String(data.fadeDurationMs || 500)}
          onChange={(e) => onChange({ ...data, fadeDurationMs: parseInt(e.target.value) || 500 })}
        />
      )}
      <Select
        label={t('stream.shuffleMode', { defaultValue: 'Shuffle Mode' })}
        value={data.shuffleMode}
        onChange={(e) => onChange({ ...data, shuffleMode: e.target.value as 'none' | 'all' | 'repeat-one' })}
        options={[
          { value: 'none', label: t('stream.shuffleNone', { defaultValue: 'None' }) },
          { value: 'all', label: t('stream.shuffleAll', { defaultValue: 'Shuffle All' }) },
          { value: 'repeat-one', label: t('stream.repeatOne', { defaultValue: 'Repeat One' }) },
        ]}
      />
      <div className="flex items-center justify-between">
        <span className="text-sm">{t('stream.captureAudio', { defaultValue: 'Capture Audio' })}</span>
        <Toggle
          checked={data.captureAudio}
          onChange={(checked) => onChange({ ...data, captureAudio: checked })}
        />
      </div>
    </div>
  );
});

MediaPlaylistForm.displayName = 'MediaPlaylistForm';
