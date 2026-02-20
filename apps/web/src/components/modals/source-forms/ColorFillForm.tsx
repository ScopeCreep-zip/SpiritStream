import React from 'react';
import { useTranslation } from 'react-i18next';
import { Input } from '@/components/ui/Input';
import type { ColorSource } from '@/types/source';

interface ColorFillFormProps {
  data: ColorSource;
  onChange: (data: ColorSource) => void;
}

export const ColorFillForm = React.memo(({ data, onChange }: ColorFillFormProps) => {
  const { t } = useTranslation();

  const presetColors = ['#000000', '#FFFFFF', '#EF4444', '#22C55E', '#3B82F6', '#7C3AED', '#EC4899', '#F59E0B'];

  return (
    <div className="flex flex-col gap-4">
      <Input
        label={t('stream.sourceName', { defaultValue: 'Source Name' })}
        value={data.name}
        onChange={(e) => onChange({ ...data, name: e.target.value })}
        placeholder="Color Fill"
      />
      <div className="space-y-2">
        <label className="text-sm font-medium">{t('stream.color', { defaultValue: 'Color' })}</label>
        <div className="flex gap-2">
          <input
            type="color"
            value={data.color}
            onChange={(e) => onChange({ ...data, color: e.target.value })}
            className="w-12 h-12 rounded cursor-pointer border border-border bg-transparent"
          />
          <Input
            value={data.color}
            onChange={(e) => onChange({ ...data, color: e.target.value })}
            placeholder="#7C3AED"
            className="flex-1"
          />
        </div>
      </div>
      <div className="space-y-2">
        <label className="text-sm font-medium">{t('stream.presetColors', { defaultValue: 'Preset Colors' })}</label>
        <div className="flex gap-2 flex-wrap">
          {presetColors.map((c) => (
            <button
              key={c}
              type="button"
              onClick={() => onChange({ ...data, color: c })}
              className={`w-8 h-8 rounded border-2 transition-colors ${
                data.color.toLowerCase() === c.toLowerCase()
                  ? 'border-primary'
                  : 'border-transparent hover:border-[var(--border-strong)]'
              }`}
              style={{ backgroundColor: c }}
              title={c}
            />
          ))}
        </div>
      </div>
      <div className="space-y-2">
        <label className="text-sm font-medium">
          {t('stream.opacity', { defaultValue: 'Opacity' })}: {Math.round(data.opacity * 100)}%
        </label>
        <input
          type="range"
          min="0"
          max="100"
          value={data.opacity * 100}
          onChange={(e) => onChange({ ...data, opacity: Number(e.target.value) / 100 })}
          className="w-full h-2 bg-[var(--bg-sunken)] rounded-lg appearance-none cursor-pointer accent-primary"
        />
      </div>
    </div>
  );
});

ColorFillForm.displayName = 'ColorFillForm';
