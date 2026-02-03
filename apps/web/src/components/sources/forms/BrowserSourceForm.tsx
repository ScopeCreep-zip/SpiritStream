/**
 * Browser Source Form
 * Configuration form for web page/widget sources
 */
import { useTranslation } from 'react-i18next';
import { Input } from '@/components/ui/Input';
import type { BrowserSource } from '@/types/source';
import type { SourceFormProps } from './types';

export function BrowserSourceForm({ data, onChange }: SourceFormProps<BrowserSource>) {
  const { t } = useTranslation();

  return (
    <div className="flex flex-col gap-4">
      <Input
        label={t('stream.sourceName', { defaultValue: 'Source Name' })}
        value={data.name}
        onChange={(e) => onChange({ ...data, name: e.target.value })}
        placeholder="Browser Source"
      />
      <Input
        label={t('stream.url', { defaultValue: 'URL' })}
        value={data.url}
        onChange={(e) => onChange({ ...data, url: e.target.value })}
        placeholder="https://example.com"
        helper={t('stream.browserUrlHelper', { defaultValue: 'Enter the web page URL or local HTML file path' })}
      />
      <div className="grid grid-cols-2 gap-4">
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
      <Input
        label={t('stream.refreshInterval', { defaultValue: 'Auto-refresh Interval (seconds)' })}
        type="number"
        value={String(data.refreshInterval || 0)}
        onChange={(e) => onChange({ ...data, refreshInterval: parseInt(e.target.value) || undefined })}
        helper={t('stream.refreshIntervalHelper', { defaultValue: '0 = manual refresh only' })}
      />
      <div>
        <label className="block text-sm font-medium mb-2">
          {t('stream.customCss', { defaultValue: 'Custom CSS (optional)' })}
        </label>
        <textarea
          value={data.customCss || ''}
          onChange={(e) => onChange({ ...data, customCss: e.target.value || undefined })}
          className="w-full h-20 px-3 py-2 rounded-lg border border-[var(--border)] bg-[var(--bg-input)] text-[var(--text)] font-mono text-sm resize-none focus:outline-none focus:border-[var(--primary)]"
          placeholder="body { background: transparent; }"
        />
      </div>
    </div>
  );
}
