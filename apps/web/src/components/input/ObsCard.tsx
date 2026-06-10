import React from 'react';
import { useTranslation } from 'react-i18next';
import { Monitor, Settings } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { useObsStore } from '@/stores/obsStore';
import { cn } from '@/lib/cn';

interface ObsCardProps {
  /** Opens the OBS configuration modal. */
  onConfigure: () => void;
}

export function ObsCard({ onConfigure }: ObsCardProps): React.ReactElement {
  const { t } = useTranslation();
  const connectionStatus = useObsStore((s) => s.connectionStatus);
  const streamStatus = useObsStore((s) => s.streamStatus);
  const isConnected = connectionStatus === 'connected';

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Monitor className="w-4 h-4 text-text-tertiary" aria-hidden="true" />
          {t('input.obs.title', { defaultValue: 'OBS' })}
        </CardTitle>
      </CardHeader>
      <CardBody className="p-4 flex flex-col gap-3">
        <div className="flex items-center gap-2 text-sm">
          <span
            className={cn(
              'w-2 h-2 rounded-full',
              isConnected ? 'bg-success-text' : 'bg-text-muted'
            )}
            aria-hidden="true"
          />
          <span className="text-text-primary">
            {t(`input.obs.status.${connectionStatus}`, {
              defaultValue: connectionStatus,
            })}
          </span>
          {isConnected && streamStatus !== 'unknown' && (
            <span className="text-text-tertiary text-xs ms-auto">
              {t(`input.obs.streamStatus.${streamStatus}`, {
                defaultValue: streamStatus,
              })}
            </span>
          )}
        </div>
        <Button variant="outline" size="sm" onClick={onConfigure} className="self-start">
          <Settings className="w-4 h-4" aria-hidden="true" />
          {t('common.configure', { defaultValue: 'Configure' })}
        </Button>
      </CardBody>
    </Card>
  );
}
