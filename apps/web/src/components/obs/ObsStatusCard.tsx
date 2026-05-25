import React from 'react';
import { useTranslation } from 'react-i18next';
import { Wifi, WifiOff, Loader2, Square, AlertCircle, CheckCircle2 } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import type { ObsConnectionStatus, ObsStreamStatus } from '@spiritstream/types';

interface ObsStatusCardProps {
  connectionStatus: ObsConnectionStatus;
  streamStatus: ObsStreamStatus;
  errorMessage: string | null;
  obsVersion: string | null;
  websocketVersion: string | null;
  isLoading: boolean;
  onConnect: () => void;
  onDisconnect: () => void;
}

function statusIcon(status: ObsConnectionStatus): React.ReactElement {
  switch (status) {
    case 'connected':
      return <Wifi className="w-4 h-4 text-status-live" />;
    case 'connecting':
      return <Loader2 className="w-4 h-4 text-status-connecting animate-spin" />;
    case 'error':
      return <AlertCircle className="w-4 h-4 text-status-error" />;
    default:
      return <WifiOff className="w-4 h-4 text-text-tertiary" />;
  }
}

export function ObsStatusCard({
  connectionStatus,
  streamStatus,
  errorMessage,
  obsVersion,
  websocketVersion,
  isLoading,
  onConnect,
  onDisconnect,
}: ObsStatusCardProps): React.ReactElement {
  const { t } = useTranslation();
  const isConnected = connectionStatus === 'connected';
  const isConnecting = connectionStatus === 'connecting';

  const statusText = (() => {
    switch (connectionStatus) {
      case 'connected':
        return t('obs.connected');
      case 'connecting':
        return t('obs.connecting');
      case 'error':
        return t('obs.error');
      default:
        return t('obs.disconnected');
    }
  })();

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t('obs.status')}</CardTitle>
      </CardHeader>
      <CardBody className="space-y-4">
        <div className="flex items-center justify-between p-3 rounded-lg bg-bg-base">
          <div className="flex items-center gap-3">
            {statusIcon(connectionStatus)}
            <div>
              <div className="text-sm font-medium text-text-primary">{statusText}</div>
              {isConnected && obsVersion && (
                <div className="text-xs text-text-tertiary">
                  {t('obs.versionInfo', { obsVersion, wsVersion: websocketVersion })}
                </div>
              )}
            </div>
          </div>
          {isConnected ? (
            <Button variant="outline" onClick={onDisconnect} disabled={isLoading}>
              <WifiOff className="w-4 h-4" />
              {t('obs.disconnect')}
            </Button>
          ) : (
            <Button variant="primary" onClick={onConnect} disabled={isConnecting || isLoading}>
              {isConnecting ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                <Wifi className="w-4 h-4" />
              )}
              {t('obs.connect')}
            </Button>
          )}
        </div>

        {errorMessage && (
          <div className="flex items-start gap-2 p-3 rounded-lg bg-status-error/10 border border-status-error/20">
            <AlertCircle className="w-4 h-4 text-status-error flex-shrink-0 mt-0.5" />
            <p className="text-sm text-status-error">{errorMessage}</p>
          </div>
        )}

        {isConnected && (
          <div className="flex items-center gap-2 p-3 rounded-lg bg-bg-base">
            {streamStatus === 'active' ? (
              <CheckCircle2 className="w-4 h-4 text-status-live" />
            ) : (
              <Square className="w-4 h-4 text-text-tertiary" />
            )}
            <span className="text-sm text-text-primary">
              {streamStatus === 'active' ? t('obs.streaming') : t('obs.notStreaming')}
            </span>
          </div>
        )}
      </CardBody>
    </Card>
  );
}
