/**
 * Color Source Editor
 * Extracted from PropertiesPanel — edits color and opacity for ColorSource
 */
import { Input } from '@/components/ui/Input';
import type { ColorSource, Source } from '@/types/source';
import { toast, formatError } from '@/hooks/useToast';
import type { SourceEditorProps } from './types';

export function ColorSourceEditor({
  source,
  profileName,
  updateSource,
  updateCurrentSource,
  t,
}: SourceEditorProps<ColorSource>) {
  const presetColors = ['#000000', '#FFFFFF', '#EF4444', '#22C55E', '#3B82F6', '#7C3AED', '#EC4899', '#F59E0B'];

  const handleUpdate = async (updates: Partial<ColorSource>) => {
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
        {t('stream.colorSettings', { defaultValue: 'Color Settings' })}
      </h4>
      <div className="space-y-3">
        <div className="flex gap-2">
          <input
            type="color"
            value={source.color}
            onChange={(e) => handleUpdate({ color: e.target.value })}
            className="w-12 h-10 rounded cursor-pointer border border-[var(--border-default)] bg-transparent"
          />
          <Input
            value={source.color}
            onChange={(e) => handleUpdate({ color: e.target.value })}
            placeholder="#7C3AED"
            className="flex-1"
          />
        </div>
        <div className="flex gap-1.5 flex-wrap">
          {presetColors.map((c) => (
            <button
              key={c}
              type="button"
              onClick={() => handleUpdate({ color: c })}
              className={`w-6 h-6 rounded border-2 transition-colors ${
                source.color.toLowerCase() === c.toLowerCase()
                  ? 'border-primary'
                  : 'border-transparent hover:border-[var(--border-strong)]'
              }`}
              style={{ backgroundColor: c }}
              title={c}
            />
          ))}
        </div>
        <div className="space-y-1">
          <label className="text-xs text-[var(--text-muted)]">
            {t('stream.opacity', { defaultValue: 'Opacity' })}: {Math.round(source.opacity * 100)}%
          </label>
          <input
            type="range"
            min="0"
            max="100"
            value={source.opacity * 100}
            onChange={(e) => handleUpdate({ opacity: Number(e.target.value) / 100 })}
            className="w-full h-2 bg-[var(--bg-sunken)] rounded-lg appearance-none cursor-pointer accent-primary"
          />
        </div>
      </div>
    </div>
  );
}
