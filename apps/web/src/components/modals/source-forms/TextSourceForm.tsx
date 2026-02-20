import React from 'react';
import { useTranslation } from 'react-i18next';
import { Input } from '@/components/ui/Input';
import { Select, SelectOption } from '@/components/ui/Select';
import type { TextSource } from '@/types/source';

interface TextSourceFormProps {
  data: TextSource;
  onChange: (data: TextSource) => void;
}

export const TextSourceForm = React.memo(({ data, onChange }: TextSourceFormProps) => {
  const { t } = useTranslation();

  const fontOptions: SelectOption[] = [
    { value: 'Arial', label: 'Arial' },
    { value: 'Helvetica', label: 'Helvetica' },
    { value: 'Times New Roman', label: 'Times New Roman' },
    { value: 'Georgia', label: 'Georgia' },
    { value: 'Verdana', label: 'Verdana' },
    { value: 'Courier New', label: 'Courier New' },
    { value: 'Impact', label: 'Impact' },
  ];

  return (
    <div className="flex flex-col gap-4">
      <Input
        label={t('stream.sourceName', { defaultValue: 'Source Name' })}
        value={data.name}
        onChange={(e) => onChange({ ...data, name: e.target.value })}
        placeholder="Text"
      />
      <div className="space-y-2">
        <label className="text-sm font-medium">{t('stream.textContent', { defaultValue: 'Text Content' })}</label>
        <textarea
          value={data.content}
          onChange={(e) => onChange({ ...data, content: e.target.value })}
          className="w-full h-24 px-3 py-2 bg-[var(--bg-sunken)] border border-border rounded-lg resize-none focus:outline-none focus:ring-2 focus:ring-primary/50 focus:border-primary"
          placeholder={t('stream.enterText', { defaultValue: 'Enter your text...' })}
        />
      </div>
      <div className="grid grid-cols-2 gap-3">
        <Select
          label={t('stream.font', { defaultValue: 'Font' })}
          value={data.fontFamily}
          onChange={(e) => onChange({ ...data, fontFamily: e.target.value })}
          options={fontOptions}
        />
        <Input
          label={t('stream.fontSize', { defaultValue: 'Size' })}
          type="number"
          value={String(data.fontSize)}
          onChange={(e) => onChange({ ...data, fontSize: parseInt(e.target.value) || 48 })}
        />
      </div>
      <div className="flex gap-2">
        <label className="text-sm font-medium mr-2">{t('stream.style', { defaultValue: 'Style' })}</label>
        <button
          type="button"
          onClick={() => onChange({ ...data, fontWeight: data.fontWeight === 'bold' ? 'normal' : 'bold' })}
          className={`px-4 py-2 rounded font-bold transition-colors ${
            data.fontWeight === 'bold'
              ? 'bg-primary text-primary-foreground'
              : 'bg-[var(--bg-sunken)] hover:bg-[var(--bg-elevated)]'
          }`}
        >
          B
        </button>
        <button
          type="button"
          onClick={() => onChange({ ...data, fontStyle: data.fontStyle === 'italic' ? 'normal' : 'italic' })}
          className={`px-4 py-2 rounded italic transition-colors ${
            data.fontStyle === 'italic'
              ? 'bg-primary text-primary-foreground'
              : 'bg-[var(--bg-sunken)] hover:bg-[var(--bg-elevated)]'
          }`}
        >
          I
        </button>
      </div>
      <div className="space-y-2">
        <label className="text-sm font-medium">{t('stream.alignment', { defaultValue: 'Alignment' })}</label>
        <div className="flex gap-1 bg-[var(--bg-sunken)] p-1 rounded-lg">
          {(['left', 'center', 'right'] as const).map((align) => (
            <button
              key={align}
              type="button"
              onClick={() => onChange({ ...data, textAlign: align })}
              className={`flex-1 px-3 py-1.5 text-sm font-medium rounded-md transition-colors ${
                data.textAlign === align
                  ? 'bg-[var(--bg-base)] text-[var(--text-primary)] shadow-sm'
                  : 'text-[var(--text-muted)] hover:text-[var(--text-secondary)]'
              }`}
            >
              {align.charAt(0).toUpperCase() + align.slice(1)}
            </button>
          ))}
        </div>
      </div>
      <div className="grid grid-cols-2 gap-3">
        <div className="space-y-2">
          <label className="text-sm font-medium">{t('stream.textColor', { defaultValue: 'Text Color' })}</label>
          <input
            type="color"
            value={data.textColor}
            onChange={(e) => onChange({ ...data, textColor: e.target.value })}
            className="w-full h-10 rounded cursor-pointer border border-border bg-transparent"
          />
        </div>
        <div className="space-y-2">
          <label className="text-sm font-medium">{t('stream.backgroundColor', { defaultValue: 'Background' })}</label>
          <div className="flex gap-2">
            <input
              type="color"
              value={data.backgroundColor || '#000000'}
              onChange={(e) => onChange({ ...data, backgroundColor: e.target.value })}
              className="flex-1 h-10 rounded cursor-pointer border border-border bg-transparent"
            />
            <button
              type="button"
              onClick={() => onChange({ ...data, backgroundColor: undefined })}
              className={`px-3 rounded border transition-colors ${
                !data.backgroundColor
                  ? 'border-primary text-primary'
                  : 'border-border text-muted hover:border-[var(--border-strong)]'
              }`}
              title={t('stream.noBackground', { defaultValue: 'No background' })}
            >
              ✕
            </button>
          </div>
        </div>
      </div>
    </div>
  );
});

TextSourceForm.displayName = 'TextSourceForm';
