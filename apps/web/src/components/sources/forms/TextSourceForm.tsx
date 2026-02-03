/**
 * Text Source Form
 * Configuration form for text overlay sources
 */
import { useTranslation } from 'react-i18next';
import { Input } from '@/components/ui/Input';
import { Select } from '@/components/ui/Select';
import { Toggle } from '@/components/ui/Toggle';
import type { TextSource } from '@/types/source';
import type { SourceFormProps } from './types';

const FONT_OPTIONS = [
  { value: 'Arial', label: 'Arial' },
  { value: 'Helvetica', label: 'Helvetica' },
  { value: 'Times New Roman', label: 'Times New Roman' },
  { value: 'Georgia', label: 'Georgia' },
  { value: 'Verdana', label: 'Verdana' },
  { value: 'Courier New', label: 'Courier New' },
  { value: 'Impact', label: 'Impact' },
  { value: 'Comic Sans MS', label: 'Comic Sans MS' },
];

const ALIGN_OPTIONS = [
  { value: 'left', label: 'Left' },
  { value: 'center', label: 'Center' },
  { value: 'right', label: 'Right' },
];

const WEIGHT_OPTIONS = [
  { value: 'normal', label: 'Normal' },
  { value: 'bold', label: 'Bold' },
];

const STYLE_OPTIONS = [
  { value: 'normal', label: 'Normal' },
  { value: 'italic', label: 'Italic' },
];

export function TextSourceForm({ data, onChange }: SourceFormProps<TextSource>) {
  const { t } = useTranslation();

  return (
    <div className="flex flex-col gap-4">
      <Input
        label={t('stream.sourceName', { defaultValue: 'Source Name' })}
        value={data.name}
        onChange={(e) => onChange({ ...data, name: e.target.value })}
        placeholder="Text"
      />
      <div>
        <label className="block text-sm font-medium mb-2">
          {t('stream.textContent', { defaultValue: 'Text Content' })}
        </label>
        <textarea
          value={data.content}
          onChange={(e) => onChange({ ...data, content: e.target.value })}
          className="w-full h-24 px-3 py-2 rounded-lg border border-[var(--border)] bg-[var(--bg-input)] text-[var(--text)] resize-none focus:outline-none focus:border-[var(--primary)]"
          placeholder="Enter text..."
        />
      </div>
      <div className="grid grid-cols-2 gap-4">
        <Select
          label={t('stream.font', { defaultValue: 'Font' })}
          value={data.fontFamily}
          onChange={(e) => onChange({ ...data, fontFamily: e.target.value })}
          options={FONT_OPTIONS}
        />
        <Input
          label={t('stream.fontSize', { defaultValue: 'Font Size' })}
          type="number"
          value={String(data.fontSize)}
          onChange={(e) => onChange({ ...data, fontSize: parseInt(e.target.value) || 48 })}
        />
      </div>
      <div className="grid grid-cols-2 gap-4">
        <Select
          label={t('stream.fontWeight', { defaultValue: 'Weight' })}
          value={data.fontWeight}
          onChange={(e) => onChange({ ...data, fontWeight: e.target.value as 'normal' | 'bold' })}
          options={WEIGHT_OPTIONS}
        />
        <Select
          label={t('stream.fontStyle', { defaultValue: 'Style' })}
          value={data.fontStyle}
          onChange={(e) => onChange({ ...data, fontStyle: e.target.value as 'normal' | 'italic' })}
          options={STYLE_OPTIONS}
        />
      </div>
      <div className="grid grid-cols-2 gap-4">
        <div>
          <label className="block text-sm font-medium mb-2">
            {t('stream.textColor', { defaultValue: 'Text Color' })}
          </label>
          <div className="flex items-center gap-2">
            <input
              type="color"
              value={data.textColor}
              onChange={(e) => onChange({ ...data, textColor: e.target.value })}
              className="w-10 h-10 rounded border border-[var(--border)] cursor-pointer"
            />
            <Input
              value={data.textColor}
              onChange={(e) => onChange({ ...data, textColor: e.target.value })}
              className="flex-1"
            />
          </div>
        </div>
        <div>
          <label className="block text-sm font-medium mb-2">
            {t('stream.backgroundColor', { defaultValue: 'Background' })}
          </label>
          <div className="flex items-center gap-2">
            <input
              type="color"
              value={data.backgroundColor || '#00000000'}
              onChange={(e) => onChange({ ...data, backgroundColor: e.target.value })}
              className="w-10 h-10 rounded border border-[var(--border)] cursor-pointer"
            />
            <Input
              value={data.backgroundColor || ''}
              onChange={(e) => onChange({ ...data, backgroundColor: e.target.value || undefined })}
              placeholder="transparent"
              className="flex-1"
            />
          </div>
        </div>
      </div>
      <div className="grid grid-cols-2 gap-4">
        <Select
          label={t('stream.alignment', { defaultValue: 'Alignment' })}
          value={data.textAlign}
          onChange={(e) => onChange({ ...data, textAlign: e.target.value as 'left' | 'center' | 'right' })}
          options={ALIGN_OPTIONS}
        />
        <Input
          label={t('stream.lineHeight', { defaultValue: 'Line Height' })}
          type="number"
          step="0.1"
          value={String(data.lineHeight)}
          onChange={(e) => onChange({ ...data, lineHeight: parseFloat(e.target.value) || 1.2 })}
        />
      </div>
      <Input
        label={t('stream.padding', { defaultValue: 'Padding (px)' })}
        type="number"
        value={String(data.padding)}
        onChange={(e) => onChange({ ...data, padding: parseInt(e.target.value) || 0 })}
      />

      {/* Outline settings */}
      <div className="border-t border-[var(--border)] pt-4 mt-2">
        <div className="flex items-center justify-between mb-3">
          <span className="text-sm font-medium">{t('stream.outline', { defaultValue: 'Text Outline' })}</span>
          <Toggle
            checked={data.outline?.enabled ?? false}
            onChange={(checked) => onChange({
              ...data,
              outline: {
                enabled: checked,
                color: data.outline?.color || '#000000',
                width: data.outline?.width || 2,
              },
            })}
          />
        </div>
        {data.outline?.enabled && (
          <div className="grid grid-cols-2 gap-4">
            <Input
              label={t('stream.outlineWidth', { defaultValue: 'Width' })}
              type="number"
              value={String(data.outline.width)}
              onChange={(e) => onChange({
                ...data,
                outline: {
                  ...data.outline!,
                  width: parseInt(e.target.value) || 2,
                },
              })}
            />
            <div>
              <label className="block text-sm font-medium mb-2">
                {t('stream.outlineColor', { defaultValue: 'Color' })}
              </label>
              <input
                type="color"
                value={data.outline.color}
                onChange={(e) => onChange({
                  ...data,
                  outline: {
                    ...data.outline!,
                    color: e.target.value,
                  },
                })}
                className="w-10 h-10 rounded border border-[var(--border)] cursor-pointer"
              />
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
