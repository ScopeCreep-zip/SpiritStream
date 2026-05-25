import React from 'react';
import { useTranslation } from 'react-i18next';
import { Input } from '@/components/ui/Input';

interface RtmpInputFormProps {
  bindAddress: string;
  port: string;
  application: string;

  onBindAddressChange: (event: React.ChangeEvent<HTMLInputElement>) => void;
  onPortChange: (event: React.ChangeEvent<HTMLInputElement>) => void;
  onApplicationChange: (event: React.ChangeEvent<HTMLInputElement>) => void;

  errors: {
    bindAddress?: string;
    port?: string;
    application?: string;
  };
}

/**
 * RTMP input block — bindAddress, port, application, URL preview, and the
 * one-paragraph explainer. Pure presentation: takes the form fields, change
 * handlers, and validation errors from the parent ProfileModal.
 */
export function RtmpInputForm({
  bindAddress,
  port,
  application,
  onBindAddressChange,
  onPortChange,
  onApplicationChange,
  errors,
}: RtmpInputFormProps): React.ReactElement {
  const { t } = useTranslation();
  const tDynamic = t as (key: string, options?: { defaultValue?: string }) => string;

  return (
    <div className="p-3 bg-bg-muted rounded-lg">
      <div className="mb-3 text-sm font-medium text-text-primary">
        {t('modals.rtmpInputSettings')}
      </div>
      <div className="grid grid-cols-[1fr_100px_1fr] gap-3">
        <Input
          label={t('modals.bindAddress')}
          placeholder={t('modals.bindAddressPlaceholder')}
          value={bindAddress}
          onChange={onBindAddressChange}
          error={errors.bindAddress}
          helper={t('modals.bindAddressHelper')}
        />
        <Input
          label={t('modals.port')}
          type="number"
          placeholder={t('modals.portPlaceholder')}
          value={port}
          onChange={onPortChange}
          error={errors.port}
        />
        <Input
          label={t('modals.application')}
          placeholder={t('modals.applicationPlaceholder')}
          value={application}
          onChange={onApplicationChange}
          error={errors.application}
          helper={t('modals.applicationHelper')}
        />
      </div>
      <div className="mt-2 text-xs text-text-tertiary">
        {t('modals.rtmpUrlPreview')}: rtmp://{bindAddress}:{port}/{application}
      </div>
      <div className="mt-2 p-2 bg-bg-base rounded text-xs text-text-secondary leading-normal">
        {tDynamic('modals.profileExplanation', {
          defaultValue:
            'Configure your streaming software (OBS, etc.) to send to this RTMP URL. Encoding settings are configured in your streaming software, not in the profile. Use output groups to re-encode to different settings for different platforms.',
        })}
      </div>
    </div>
  );
}
