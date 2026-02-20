import React from 'react';
import { useTranslation } from 'react-i18next';
import { Input } from '@/components/ui/Input';
import { Toggle } from '@/components/ui/Toggle';
import type { NDISource } from '@/types/source';

interface NdiSourceFormProps {
  data: NDISource;
  onChange: (data: NDISource) => void;
}

export const NdiSourceForm = React.memo(({ data, onChange }: NdiSourceFormProps) => {
  const { t } = useTranslation();

  return (
    <div className="flex flex-col gap-4">
      <Input
        label={t('stream.sourceName', { defaultValue: 'Source Name' })}
        value={data.name}
        onChange={(e) => onChange({ ...data, name: e.target.value })}
        placeholder="NDI Source"
      />
      <Input
        label={t('stream.ndiSourceName', { defaultValue: 'NDI Source Name' })}
        value={data.sourceName}
        onChange={(e) => onChange({ ...data, sourceName: e.target.value })}
        placeholder="CAMERA-PC (OBS)"
        helper={t('stream.ndiSourceNameHelper', { defaultValue: 'Name of the NDI source on the network' })}
      />
      <Input
        label={t('stream.ipAddress', { defaultValue: 'IP Address (optional)' })}
        value={data.ipAddress || ''}
        onChange={(e) => onChange({ ...data, ipAddress: e.target.value || undefined })}
        placeholder="192.168.1.100"
        helper={t('stream.ndiIpHelper', { defaultValue: 'Leave blank to auto-discover on local network' })}
      />
      <Input
        label={t('stream.receiverName', { defaultValue: 'Receiver Name' })}
        value={data.receiverName}
        onChange={(e) => onChange({ ...data, receiverName: e.target.value })}
        placeholder="SpiritStream"
        helper={t('stream.receiverNameHelper', { defaultValue: 'How this receiver appears to NDI sources' })}
      />
      <div className="flex items-center justify-between">
        <span className="text-sm">{t('stream.captureAudio', { defaultValue: 'Capture Audio' })}</span>
        <Toggle
          checked={data.captureAudio}
          onChange={(checked) => onChange({ ...data, captureAudio: checked })}
        />
      </div>
      <div className="flex items-center justify-between">
        <div>
          <span className="text-sm">{t('stream.lowBandwidth', { defaultValue: 'Low Bandwidth Mode' })}</span>
          <p className="text-xs text-muted">
            {t('stream.lowBandwidthHelper', { defaultValue: 'Reduces quality but uses less network bandwidth' })}
          </p>
        </div>
        <Toggle
          checked={data.lowBandwidth}
          onChange={(checked) => onChange({ ...data, lowBandwidth: checked })}
        />
      </div>
      <div className="p-3 bg-[var(--bg-sunken)] rounded-lg text-sm text-muted">
        <p className="font-medium mb-1">{t('stream.ndiRequirement', { defaultValue: 'NDI Runtime Required' })}</p>
        <p>{t('stream.ndiRequirementHelper', { defaultValue: 'NDI® runtime must be installed on this system. Download from ndi.video' })}</p>
      </div>
    </div>
  );
});

NdiSourceForm.displayName = 'NdiSourceForm';
