/**
 * Color Source Form
 * Configuration form for solid color fill sources
 */
import { useTranslation } from 'react-i18next';
import { Input } from '@/components/ui/Input';
import type { ColorSource } from '@/types/source';
import type { SourceFormProps } from './types';

export function ColorSourceForm({ data, onChange }: SourceFormProps<ColorSource>) {
  const { t } = useTranslation();

  return (
    <div className="flex flex-col gap-4">
      <Input
        label={t('stream.sourceName', { defaultValue: 'Source Name' })}
        value={data.name}
        onChange={(e) => onChange({ ...data, name: e.target.value })}
        placeholder="Color Fill"
      />
      <div>
        <label className="block text-sm font-medium mb-2">
          {t('stream.color', { defaultValue: 'Color' })}
        </label>
        <div className="flex items-center gap-3">
          <input
            type="color"
            value={data.color}
            onChange={(e) => onChange({ ...data, color: e.target.value })}
            className="w-12 h-10 rounded border border-[var(--border)] cursor-pointer"
          />
          <Input
            value={data.color}
            onChange={(e) => onChange({ ...data, color: e.target.value })}
            placeholder="#000000"
            className="flex-1"
          />
        </div>
      </div>
      <div>
        <label className="block text-sm font-medium mb-2">
          {t('stream.opacity', { defaultValue: 'Opacity' })}
        </label>
        <div className="flex items-center gap-3">
          <input
            type="range"
            min="0"
            max="1"
            step="0.01"
            value={data.opacity}
            onChange={(e) => onChange({ ...data, opacity: parseFloat(e.target.value) })}
            className="flex-1"
          />
          <span className="text-sm text-muted w-12 text-right">
            {Math.round(data.opacity * 100)}%
          </span>
        </div>
      </div>
    </div>
  );
}
