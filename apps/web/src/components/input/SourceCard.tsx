import React from 'react';
import { useTranslation } from 'react-i18next';
import { Radio, Settings } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import type { Profile } from '@spiritstream/types';

interface SourceCardProps {
  profile: Profile | null;
  /** Opens the profile-edit modal (where RTMP input fields live). */
  onConfigure: () => void;
}

export function SourceCard({ profile, onConfigure }: SourceCardProps): React.ReactElement {
  const { t } = useTranslation();

  if (!profile) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Radio className="w-4 h-4 text-text-tertiary" aria-hidden="true" />
            {t('input.source.title', { defaultValue: 'Source' })}
          </CardTitle>
        </CardHeader>
        <CardBody className="p-4">
          <p className="text-sm text-text-secondary">
            {t('input.source.noProfile', { defaultValue: 'Load a profile to see input.' })}
          </p>
        </CardBody>
      </Card>
    );
  }

  // Server-computed (`RtmpInput::refresh_url`). The bind URL may carry a
  // wildcard host (`0.0.0.0` / `[::]`) which is NOT pushable — OBS would fail
  // to connect. Show the explicit IPv4 loopback an encoder actually pushes to
  // (presentation only; the relay normalizes the bind internally, and OBS is
  // auto-pointed here via SetStreamServiceSettings on connect).
  const url = profile.input.url.replace(/\/\/(?:0\.0\.0\.0|\[::\]|::)(?=[:/])/, '//127.0.0.1');

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Radio className="w-4 h-4 text-text-tertiary" aria-hidden="true" />
          {t('input.source.title', { defaultValue: 'Source' })}
        </CardTitle>
      </CardHeader>
      <CardBody className="p-4 flex flex-col gap-3">
        <dl className="text-sm">
          <dt className="text-text-tertiary text-xs uppercase tracking-wide mb-1">
            {t('input.source.endpoint', { defaultValue: 'RTMP endpoint' })}
          </dt>
          <dd className="font-mono text-text-primary text-xs break-all">{url}</dd>
        </dl>
        <Button variant="outline" size="sm" onClick={onConfigure} className="self-start">
          <Settings className="w-4 h-4" aria-hidden="true" />
          {t('common.configure', { defaultValue: 'Configure' })}
        </Button>
      </CardBody>
    </Card>
  );
}
