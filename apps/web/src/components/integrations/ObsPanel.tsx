import { useEffect, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Radio,
  Wifi,
  WifiOff,
  Loader2,
  Play,
  Square,
  Eye,
  EyeOff,
  AlertCircle,
  CheckCircle2,
} from 'lucide-react';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Input } from '@/components/ui/Input';
import { Toggle } from '@/components/ui/Toggle';
import { useObsStore } from '@/stores/obsStore';
import { toast } from '@/hooks/useToast';
import { cn } from '@/lib/cn';
import type { ObsIntegrationDirection } from '@/types/api';

const directionOptions: { value: ObsIntegrationDirection; labelKey: string; descKey: string }[] = [
  {
    value: 'disabled',
    labelKey: 'obs.directions.disabled',
    descKey: 'obs.directions.disabledDescription',
  },
  {
    value: 'obs-to-spiritstream',
    labelKey: 'obs.directions.obsToSpiritstream',
    descKey: 'obs.directions.obsToSpiritstreamDescription',
  },
  {
    value: 'spiritstream-to-obs',
    labelKey: 'obs.directions.spiritstreamToObs',
    descKey: 'obs.directions.spiritstreamToObsDescription',
  },
  {
    value: 'bidirectional',
    labelKey: 'obs.directions.bidirectional',
    descKey: 'obs.directions.bidirectionalDescription',
  },
];

export function ObsPanel() {
  const { t } = useTranslation();
  const {
    connectionStatus,
    streamStatus,
    errorMessage,
    obsVersion,
    websocketVersion,
    config,
    isLoading,
    showPassword,
    setShowPassword,
    loadState,
    loadConfig,
    updateConfig,
    connect,
    disconnect,
    startStream,
    stopStream,
  } = useObsStore();

  // Local form state
  const [host, setHost] = useState('localhost');
  const [port, setPort] = useState('4455');
  const [password, setPassword] = useState('');
  const [useAuth, setUseAuth] = useState(false);
  const [direction, setDirection] = useState<ObsIntegrationDirection>('disabled');
  const [autoConnect, setAutoConnect] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [hasPasswordSet, setHasPasswordSet] = useState(false);

  // Load initial state and config
  useEffect(() => {
    loadState();
    loadConfig();
  }, [loadState, loadConfig]);

  // Sync form with config when loaded
  useEffect(() => {
    if (config) {
      setHost(config.host || 'localhost');
      setPort(String(config.port || 4455));
      setUseAuth(config.useAuth);
      setDirection(config.direction);
      setAutoConnect(config.autoConnect);
      setHasPasswordSet(config.hasPassword || false);
      // Don't set password - it's masked
    }
  }, [config]);

  const handleSave = useCallback(async () => {
    setIsSaving(true);
    try {
      await updateConfig({
        host,
        port: parseInt(port, 10) || 4455,
        password: password || undefined,
        useAuth,
        direction,
        autoConnect,
      });
      setPassword(''); // Clear password field after save
      toast.success(t('obs.configSaved'));
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(t('obs.configSaveFailed') + ': ' + message);
    } finally {
      setIsSaving(false);
    }
  }, [host, port, password, useAuth, direction, autoConnect, updateConfig, t]);

  const handleConnect = useCallback(async () => {
    try {
      await connect();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(t('obs.connectFailed') + ': ' + message);
    }
  }, [connect, t]);

  const handleDisconnect = useCallback(async () => {
    try {
      await disconnect();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      toast.error(t('obs.disconnectFailed') + ': ' + message);
    }
  }, [disconnect, t]);

  const handleStartStream = useCallback(async () => {
    try {
      await startStream();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  }, [startStream]);

  const handleStopStream = useCallback(async () => {
    try {
      await stopStream();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  }, [stopStream]);

  const isConnected = connectionStatus === 'connected';
  const isConnecting = connectionStatus === 'connecting';
  const isStreaming = streamStatus === 'active';

  const getStatusIcon = () => {
    switch (connectionStatus) {
      case 'connected':
        return <Wifi className="w-4 h-4 text-[var(--status-live)]" />;
      case 'connecting':
        return <Loader2 className="w-4 h-4 text-[var(--status-connecting)] animate-spin" />;
      case 'error':
        return <AlertCircle className="w-4 h-4 text-[var(--status-error)]" />;
      default:
        return <WifiOff className="w-4 h-4 text-[var(--text-tertiary)]" />;
    }
  };

  const getStatusText = () => {
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
  };

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center gap-3">
        <div className="p-2 rounded-lg bg-[var(--bg-elevated)]">
          <Radio className="w-5 h-5 text-[var(--primary)]" />
        </div>
        <div>
          <h2 className="text-lg font-semibold text-[var(--text-primary)]">{t('obs.title')}</h2>
          <p className="text-sm text-[var(--text-secondary)]">{t('obs.description')}</p>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Connection Settings Card */}
        <Card>
          <CardHeader>
            <CardTitle>{t('obs.connectionSettings')}</CardTitle>
          </CardHeader>
          <CardBody className="space-y-4">
            {/* Host and Port */}
            <div className="grid grid-cols-3 gap-3">
              <div className="col-span-2">
                <Input
                  label={t('obs.host')}
                  value={host}
                  onChange={(e) => setHost(e.target.value)}
                  placeholder={t('obs.hostPlaceholder')}
                  disabled={isConnected}
                />
              </div>
              <div>
                <Input
                  label={t('obs.port')}
                  type="number"
                  value={port}
                  onChange={(e) => setPort(e.target.value)}
                  disabled={isConnected}
                />
              </div>
            </div>

            {/* Authentication Toggle */}
            <Toggle
              checked={useAuth}
              onChange={setUseAuth}
              label={t('obs.useAuthentication')}
              disabled={isConnected}
            />

            {/* Password Field */}
            {useAuth && (
              <div className="relative">
                <Input
                  label={t('obs.password')}
                  type={showPassword ? 'text' : 'password'}
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  placeholder={hasPasswordSet ? '********' : t('obs.passwordPlaceholder')}
                  disabled={isConnected}
                />
                <button
                  type="button"
                  onClick={() => setShowPassword(!showPassword)}
                  className={cn(
                    'absolute right-3 top-[34px]',
                    'p-1 rounded-md',
                    'text-[var(--text-tertiary)] hover:text-[var(--text-primary)]',
                    'transition-colors'
                  )}
                  title={showPassword ? t('obs.hidePassword') : t('obs.showPassword')}
                >
                  {showPassword ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                </button>
              </div>
            )}

            {/* Auto-connect Toggle */}
            <Toggle
              checked={autoConnect}
              onChange={setAutoConnect}
              label={t('obs.autoConnect')}
              disabled={isConnected}
            />

            {/* Save Button */}
            <Button
              variant="secondary"
              onClick={handleSave}
              disabled={isConnected || isSaving}
              className="w-full"
            >
              {isSaving ? (
                <Loader2 className="w-4 h-4 animate-spin" />
              ) : (
                t('common.save')
              )}
            </Button>
          </CardBody>
        </Card>

        {/* Status and Control Card */}
        <Card>
          <CardHeader>
            <CardTitle>{t('obs.status')}</CardTitle>
          </CardHeader>
          <CardBody className="space-y-4">
            {/* Connection Status */}
            <div className="flex items-center justify-between p-3 rounded-lg bg-[var(--bg-base)]">
              <div className="flex items-center gap-3">
                {getStatusIcon()}
                <div>
                  <div className="text-sm font-medium text-[var(--text-primary)]">
                    {getStatusText()}
                  </div>
                  {isConnected && obsVersion && (
                    <div className="text-xs text-[var(--text-tertiary)]">
                      OBS {obsVersion} / WS {websocketVersion}
                    </div>
                  )}
                </div>
              </div>
              {isConnected ? (
                <Button
                  variant="outline"
                  onClick={handleDisconnect}
                  disabled={isLoading}
                >
                  <WifiOff className="w-4 h-4" />
                  {t('obs.disconnect')}
                </Button>
              ) : (
                <Button
                  variant="primary"
                  onClick={handleConnect}
                  disabled={isConnecting || isLoading}
                >
                  {isConnecting ? (
                    <Loader2 className="w-4 h-4 animate-spin" />
                  ) : (
                    <Wifi className="w-4 h-4" />
                  )}
                  {t('obs.connect')}
                </Button>
              )}
            </div>

            {/* Error Message */}
            {errorMessage && (
              <div className="flex items-start gap-2 p-3 rounded-lg bg-[var(--status-error)]/10 border border-[var(--status-error)]/20">
                <AlertCircle className="w-4 h-4 text-[var(--status-error)] flex-shrink-0 mt-0.5" />
                <p className="text-sm text-[var(--status-error)]">{errorMessage}</p>
              </div>
            )}

            {/* Stream Control */}
            {isConnected && (
              <div className="space-y-3">
                <div className="text-sm font-medium text-[var(--text-primary)]">
                  {t('obs.streamControl')}
                </div>
                <div className="flex items-center justify-between p-3 rounded-lg bg-[var(--bg-base)]">
                  <div className="flex items-center gap-2">
                    {isStreaming ? (
                      <CheckCircle2 className="w-4 h-4 text-[var(--status-live)]" />
                    ) : (
                      <Square className="w-4 h-4 text-[var(--text-tertiary)]" />
                    )}
                    <span className="text-sm text-[var(--text-primary)]">
                      {isStreaming ? t('obs.streaming') : t('obs.notStreaming')}
                    </span>
                  </div>
                  {isStreaming ? (
                    <Button
                      variant="destructive"
                      onClick={handleStopStream}
                      disabled={isLoading}
                    >
                      <Square className="w-4 h-4" />
                      {t('obs.stopStream')}
                    </Button>
                  ) : (
                    <Button
                      variant="primary"
                      onClick={handleStartStream}
                      disabled={isLoading}
                    >
                      <Play className="w-4 h-4" />
                      {t('obs.startStream')}
                    </Button>
                  )}
                </div>
              </div>
            )}
          </CardBody>
        </Card>
      </div>

      {/* Integration Direction Card */}
      <Card>
        <CardHeader>
          <CardTitle>{t('obs.integration')}</CardTitle>
          <CardDescription>{t('obs.integrationDirectionDescription')}</CardDescription>
        </CardHeader>
        <CardBody>
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3">
            {directionOptions.map((option) => (
              <button
                key={option.value}
                type="button"
                onClick={() => {
                  setDirection(option.value);
                  // Auto-save direction changes
                  updateConfig({ direction: option.value }).catch(console.error);
                }}
                disabled={isConnected}
                className={cn(
                  'p-4 rounded-lg border text-left transition-all',
                  direction === option.value
                    ? 'border-[var(--primary)] bg-[var(--primary)]/10'
                    : 'border-[var(--border-default)] bg-[var(--bg-base)] hover:border-[var(--border-strong)]',
                  isConnected && 'opacity-50 cursor-not-allowed'
                )}
              >
                <div className="text-sm font-medium text-[var(--text-primary)]">
                  {t(option.labelKey as 'obs.directions.disabled')}
                </div>
                <div className="text-xs text-[var(--text-tertiary)] mt-1">
                  {t(option.descKey as 'obs.directions.disabledDescription')}
                </div>
              </button>
            ))}
          </div>
        </CardBody>
      </Card>
    </div>
  );
}
