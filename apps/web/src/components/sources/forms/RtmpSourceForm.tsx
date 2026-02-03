/**
 * RTMP Source Form
 * Configuration form for RTMP input sources
 */
import { useTranslation } from 'react-i18next';
import { Input } from '@/components/ui/Input';
import { Toggle } from '@/components/ui/Toggle';
import type { RtmpSource } from '@/types/source';
import type { SourceFormProps } from './types';

export function RtmpSourceForm({ data, onChange }: SourceFormProps<RtmpSource>) {
  const { t } = useTranslation();

  return (
    <div className="flex flex-col gap-4">
      <Input
        label={t('stream.sourceName', { defaultValue: 'Source Name' })}
        value={data.name}
        onChange={(e) => onChange({ ...data, name: e.target.value })}
        placeholder="RTMP Input"
      />
      <Input
        label={t('stream.bindAddress', { defaultValue: 'Bind Address' })}
        value={data.bindAddress}
        onChange={(e) => onChange({ ...data, bindAddress: e.target.value })}
        helper={t('stream.bindAddressHelper', { defaultValue: 'Use 0.0.0.0 to accept connections from any address' })}
      />
      <div className="grid grid-cols-2 gap-4">
        <Input
          label={t('stream.port', { defaultValue: 'Port' })}
          type="number"
          value={String(data.port)}
          onChange={(e) => onChange({ ...data, port: parseInt(e.target.value) || 1935 })}
        />
        <Input
          label={t('stream.application', { defaultValue: 'Application' })}
          value={data.application}
          onChange={(e) => onChange({ ...data, application: e.target.value })}
        />
      </div>
      <div className="flex items-center justify-between">
        <span className="text-sm">{t('stream.captureAudio', { defaultValue: 'Capture Audio' })}</span>
        <Toggle
          checked={data.captureAudio}
          onChange={(checked) => onChange({ ...data, captureAudio: checked })}
        />
      </div>
    </div>
  );
}
