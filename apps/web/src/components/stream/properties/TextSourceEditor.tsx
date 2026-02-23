/**
 * Text Source Editor
 * Extracted from PropertiesPanel — edits content, font, and styling for TextSource
 */
import { Input } from '@/components/ui/Input';
import { Select } from '@/components/ui/Select';
import type { TextSource, Source } from '@/types/source';
import { toast, formatError } from '@/hooks/useToast';
import type { SourceEditorProps } from './types';

export function TextSourceEditor({
  source,
  profileName,
  updateSource,
  updateCurrentSource,
  t,
}: SourceEditorProps<TextSource>) {
  const handleUpdate = async (updates: Partial<TextSource>) => {
    try {
      const updated = await updateSource(profileName, source.id, updates as Partial<Source>);
      updateCurrentSource(updated);
    } catch (err) {
      toast.error(`Failed to update: ${formatError(err)}`);
    }
  };

  return (
    <div className="space-y-4">
      <h4 className="text-xs font-medium text-[var(--text-muted)] uppercase tracking-wide">
        {t('stream.textSettings', { defaultValue: 'Text Settings' })}
      </h4>
      <div className="space-y-3">
        {/* Text content */}
        <div>
          <label className="text-xs text-[var(--text-muted)] mb-1 block">
            {t('stream.content', { defaultValue: 'Content' })}
          </label>
          <textarea
            value={source.content}
            onChange={(e) => handleUpdate({ content: e.target.value })}
            className="w-full h-20 px-3 py-2 bg-[var(--bg-sunken)] border border-[var(--border-default)] rounded-lg resize-none text-sm focus:outline-none focus:ring-2 focus:ring-primary/50"
            placeholder={t('stream.enterText', { defaultValue: 'Enter text...' })}
          />
        </div>

        {/* Font controls */}
        <div className="grid grid-cols-2 gap-2">
          <Select
            label={t('stream.font', { defaultValue: 'Font' })}
            value={source.fontFamily}
            onChange={(e) => handleUpdate({ fontFamily: e.target.value })}
            options={[
              { value: 'Arial', label: 'Arial' },
              { value: 'Helvetica', label: 'Helvetica' },
              { value: 'Times New Roman', label: 'Times' },
              { value: 'Georgia', label: 'Georgia' },
              { value: 'Verdana', label: 'Verdana' },
              { value: 'Impact', label: 'Impact' },
            ]}
          />
          <Input
            label={t('stream.size', { defaultValue: 'Size' })}
            type="number"
            value={source.fontSize}
            onChange={(e) => handleUpdate({ fontSize: parseInt(e.target.value) || 48 })}
          />
        </div>

        {/* Style toggles */}
        <div className="flex gap-1">
          <button
            type="button"
            onClick={() => handleUpdate({ fontWeight: source.fontWeight === 'bold' ? 'normal' : 'bold' })}
            className={`px-3 py-1.5 rounded text-sm font-bold transition-colors ${
              source.fontWeight === 'bold'
                ? 'bg-primary text-primary-foreground'
                : 'bg-[var(--bg-sunken)] hover:bg-[var(--bg-elevated)]'
            }`}
          >
            B
          </button>
          <button
            type="button"
            onClick={() => handleUpdate({ fontStyle: source.fontStyle === 'italic' ? 'normal' : 'italic' })}
            className={`px-3 py-1.5 rounded text-sm italic transition-colors ${
              source.fontStyle === 'italic'
                ? 'bg-primary text-primary-foreground'
                : 'bg-[var(--bg-sunken)] hover:bg-[var(--bg-elevated)]'
            }`}
          >
            I
          </button>
          <div className="flex-1" />
          {(['left', 'center', 'right'] as const).map((align) => (
            <button
              key={align}
              type="button"
              onClick={() => handleUpdate({ textAlign: align })}
              className={`px-3 py-1.5 rounded text-xs transition-colors ${
                source.textAlign === align
                  ? 'bg-primary text-primary-foreground'
                  : 'bg-[var(--bg-sunken)] hover:bg-[var(--bg-elevated)]'
              }`}
            >
              {align.charAt(0).toUpperCase()}
            </button>
          ))}
        </div>

        {/* Colors */}
        <div className="grid grid-cols-2 gap-2">
          <div>
            <label className="text-xs text-[var(--text-muted)] mb-1 block">
              {t('stream.textColor', { defaultValue: 'Text' })}
            </label>
            <input
              type="color"
              value={source.textColor}
              onChange={(e) => handleUpdate({ textColor: e.target.value })}
              className="w-full h-8 rounded cursor-pointer border border-[var(--border-default)] bg-transparent"
            />
          </div>
          <div>
            <label className="text-xs text-[var(--text-muted)] mb-1 block">
              {t('stream.background', { defaultValue: 'Background' })}
            </label>
            <div className="flex gap-1">
              <input
                type="color"
                value={source.backgroundColor || '#000000'}
                onChange={(e) => handleUpdate({ backgroundColor: e.target.value })}
                className="flex-1 h-8 rounded cursor-pointer border border-[var(--border-default)] bg-transparent"
              />
              <button
                type="button"
                onClick={() => handleUpdate({ backgroundColor: undefined })}
                className={`px-2 h-8 rounded border text-xs transition-colors ${
                  !source.backgroundColor
                    ? 'border-primary text-primary'
                    : 'border-[var(--border-default)] text-[var(--text-muted)] hover:border-[var(--border-strong)]'
                }`}
                title={t('stream.noBackground', { defaultValue: 'No background' })}
              >
                &#x2715;
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
