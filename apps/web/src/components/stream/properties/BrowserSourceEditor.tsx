/**
 * Browser Source Editor
 * Extracted from PropertiesPanel — edits URL, dimensions, and refresh for BrowserSource
 */
import { useState, useEffect } from 'react';
import { RefreshCcw } from 'lucide-react';
import { Input } from '@/components/ui/Input';
import type { BrowserSource, Source } from '@/types/source';
import { toast, formatError } from '@/hooks/useToast';
import { blurOnEnter } from '@/utils/inputHandlers';
import type { SourceEditorProps } from './types';

export function BrowserSourceEditor({
  source,
  profileName,
  updateSource,
  updateCurrentSource,
  t,
}: SourceEditorProps<BrowserSource>) {
  // Local state for editing - prevents API calls on every keystroke
  const [url, setUrl] = useState(source.url);
  const [width, setWidth] = useState(source.width);
  const [height, setHeight] = useState(source.height);

  // Sync local state with source when it changes externally
  useEffect(() => {
    setUrl(source.url);
    setWidth(source.width);
    setHeight(source.height);
  }, [source.url, source.width, source.height]);

  const handleUpdate = async (updates: Partial<BrowserSource>) => {
    try {
      const updated = await updateSource(profileName, source.id, updates as Partial<Source>);
      updateCurrentSource(updated);
    } catch (err) {
      toast.error(`Failed to update: ${formatError(err)}`);
      // Reset local state on error
      setUrl(source.url);
      setWidth(source.width);
      setHeight(source.height);
    }
  };

  const handleRefresh = () => {
    handleUpdate({ refreshToken: crypto.randomUUID() });
  };

  return (
    <div className="space-y-4">
      <h4 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wide">
        {t('stream.browserSettings', { defaultValue: 'Browser Settings' })}
      </h4>
      <div className="space-y-3">
        <Input
          label={t('stream.url', { defaultValue: 'URL' })}
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          onBlur={() => {
            if (url !== source.url) {
              handleUpdate({ url });
            }
          }}
          onKeyDown={blurOnEnter}
          placeholder="https://example.com"
        />
        <div className="grid grid-cols-2 gap-2">
          <Input
            label={t('stream.width', { defaultValue: 'Width' })}
            type="number"
            value={width}
            onChange={(e) => setWidth(parseInt(e.target.value) || 1920)}
            onBlur={() => {
              if (width !== source.width) {
                handleUpdate({ width });
              }
            }}
          />
          <Input
            label={t('stream.height', { defaultValue: 'Height' })}
            type="number"
            value={height}
            onChange={(e) => setHeight(parseInt(e.target.value) || 1080)}
            onBlur={() => {
              if (height !== source.height) {
                handleUpdate({ height });
              }
            }}
          />
        </div>
        <button
          type="button"
          onClick={handleRefresh}
          className="flex items-center justify-center gap-2 w-full px-3 py-2 bg-[var(--bg-sunken)] hover:bg-[var(--bg-elevated)] rounded-lg text-sm transition-colors"
        >
          <RefreshCcw className="w-4 h-4" />
          {t('stream.refreshPage', { defaultValue: 'Refresh Page' })}
        </button>
      </div>
    </div>
  );
}
