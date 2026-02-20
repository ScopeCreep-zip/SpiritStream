import React from 'react';
import { useTranslation } from 'react-i18next';
import { Input } from '@/components/ui/Input';
import type { BrowserSource } from '@/types/source';

interface BrowserSourceFormProps {
  data: BrowserSource;
  onChange: (data: BrowserSource) => void;
}

export const BrowserSourceForm = React.memo(({ data, onChange }: BrowserSourceFormProps) => {
  const { t } = useTranslation();

  const dimensionPresets = [
    { label: '1080p', w: 1920, h: 1080 },
    { label: '720p', w: 1280, h: 720 },
    { label: '480p', w: 854, h: 480 },
  ];

  return (
    <div className="flex flex-col gap-4">
      <Input
        label={t('stream.sourceName', { defaultValue: 'Source Name' })}
        value={data.name}
        onChange={(e) => onChange({ ...data, name: e.target.value })}
        placeholder="Browser"
      />
      <div className="space-y-2">
        <Input
          label={t('stream.url', { defaultValue: 'URL' })}
          value={data.url}
          onChange={(e) => onChange({ ...data, url: e.target.value })}
          placeholder="https://example.com"
        />
        <p className="text-xs text-muted">
          {t('stream.browserUrlHelper', { defaultValue: 'Note: Some sites block iframe embedding' })}
        </p>
      </div>
      <div className="space-y-2">
        <label className="text-sm font-medium">{t('stream.dimensions', { defaultValue: 'Dimensions' })}</label>
        <div className="flex gap-2">
          {dimensionPresets.map((preset) => (
            <button
              key={preset.label}
              type="button"
              onClick={() => onChange({ ...data, width: preset.w, height: preset.h })}
              className={`px-3 py-1.5 rounded text-sm transition-colors ${
                data.width === preset.w && data.height === preset.h
                  ? 'bg-primary text-primary-foreground'
                  : 'bg-[var(--bg-sunken)] hover:bg-[var(--bg-elevated)]'
              }`}
            >
              {preset.label}
            </button>
          ))}
        </div>
        <div className="grid grid-cols-2 gap-2 mt-2">
          <Input
            label={t('stream.width', { defaultValue: 'Width' })}
            type="number"
            value={String(data.width)}
            onChange={(e) => onChange({ ...data, width: parseInt(e.target.value) || 1920 })}
          />
          <Input
            label={t('stream.height', { defaultValue: 'Height' })}
            type="number"
            value={String(data.height)}
            onChange={(e) => onChange({ ...data, height: parseInt(e.target.value) || 1080 })}
          />
        </div>
      </div>
    </div>
  );
});

BrowserSourceForm.displayName = 'BrowserSourceForm';
