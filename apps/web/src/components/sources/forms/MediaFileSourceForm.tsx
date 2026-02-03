/**
 * Media File Source Form
 * Configuration form for local media file sources
 */
import { useTranslation } from 'react-i18next';
import { FolderOpen } from 'lucide-react';
import { Input } from '@/components/ui/Input';
import { Button } from '@/components/ui/Button';
import { Toggle } from '@/components/ui/Toggle';
import type { MediaFileSource } from '@/types/source';
import type { SourceFormProps } from './types';

export interface MediaFileSourceFormProps extends SourceFormProps<MediaFileSource> {
  onBrowseFile: () => void;
}

export function MediaFileSourceForm({ data, onChange, onBrowseFile }: MediaFileSourceFormProps) {
  const { t } = useTranslation();

  return (
    <div className="flex flex-col gap-4">
      <Input
        label={t('stream.sourceName', { defaultValue: 'Source Name' })}
        value={data.name}
        onChange={(e) => onChange({ ...data, name: e.target.value })}
        placeholder="Media File"
      />
      <div className="flex items-end gap-2">
        <div className="flex-1">
          <Input
            label={t('stream.filePath', { defaultValue: 'File Path' })}
            value={data.filePath}
            onChange={(e) => onChange({ ...data, filePath: e.target.value })}
            placeholder="/path/to/video.mp4"
          />
        </div>
        <Button variant="secondary" className="h-10 px-3" onClick={onBrowseFile}>
          <FolderOpen className="w-4 h-4" />
        </Button>
      </div>
      <div className="flex items-center justify-between">
        <span className="text-sm">{t('stream.loopPlayback', { defaultValue: 'Loop Playback' })}</span>
        <Toggle
          checked={data.loopPlayback}
          onChange={(checked) => onChange({ ...data, loopPlayback: checked })}
        />
      </div>
      <div className="flex items-center justify-between">
        <span className="text-sm">{t('stream.audioOnly', { defaultValue: 'Audio Only' })}</span>
        <Toggle
          checked={data.audioOnly ?? false}
          onChange={(checked) => onChange({ ...data, audioOnly: checked })}
        />
      </div>
      <div className="flex items-center justify-between">
        <span className="text-sm">{t('stream.captureAudio', { defaultValue: 'Capture Audio' })}</span>
        <Toggle
          checked={data.captureAudio ?? true}
          onChange={(checked) => onChange({ ...data, captureAudio: checked })}
        />
      </div>
    </div>
  );
}
