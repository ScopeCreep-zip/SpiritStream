import { useEffect, useState, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Radio } from 'lucide-react';
import { useObsStore } from '@/stores/obsStore';
import { useProfileStore } from '@/stores/profileStore';
import { toast } from '@/hooks/useToast';
import { logger } from '@/lib/logger';
import { clientConfig } from '@/lib/constants';
import { ObsConnectionForm } from '@/components/obs/ObsConnectionForm';
import { ObsStatusCard } from '@/components/obs/ObsStatusCard';
import { ObsDirectionSelector } from '@/components/obs/ObsDirectionSelector';
import type { ObsIntegrationDirection } from '@spiritstream/types';

/**
 * OBS integration orchestrator. Owns the form state, debounced auto-save,
 * unmount-flush, and store wiring. Renders three focused cards (connection
 * form, status, direction selector) — each is its own component under
 * `components/obs/`.
 */
export function ObsPanel() {
  const { t } = useTranslation();
  const currentProfile = useProfileStore((state) => state.current);
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
    updateConfig,
    connect,
    disconnect,
  } = useObsStore();

  // Local form state — mirrors the obsStore config; saved via autoSave / immediate update.
  const [host, setHost] = useState('localhost');
  const [port, setPort] = useState('4455');
  const [password, setPassword] = useState('');
  const [useAuth, setUseAuth] = useState(false);
  const [direction, setDirection] = useState<ObsIntegrationDirection>('disabled');
  const [autoConnect, setAutoConnect] = useState(false);

  // Debounced save infrastructure.
  const saveTimeoutRef = useRef<number | null>(null);
  const pendingUpdatesRef = useRef<Parameters<typeof updateConfig>[0] | null>(null);

  // Load initial OBS connection state (not config — that comes from profile).
  useEffect(() => {
    loadState();
  }, [loadState]);

  // Sync form with config when loaded.
  useEffect(() => {
    if (config) {
      setHost(config.host || 'localhost');
      setPort(String(config.port || 4455));
      setPassword(config.password || '');
      setUseAuth(config.useAuth);
      setDirection(config.direction);
      setAutoConnect(config.autoConnect);
    }
  }, [config]);

  // Flush pending saves on unmount.
  useEffect(() => {
    return () => {
      if (saveTimeoutRef.current) {
        window.clearTimeout(saveTimeoutRef.current);
      }
      if (pendingUpdatesRef.current) {
        updateConfig(pendingUpdatesRef.current).catch((error) => {
          logger.error('Failed to flush OBS config on unmount:', error);
        });
        pendingUpdatesRef.current = null;
      }
    };
  }, [updateConfig]);

  // Auto-save with debounce.
  const autoSave = useCallback(
    (updates: Parameters<typeof updateConfig>[0]) => {
      pendingUpdatesRef.current = { ...pendingUpdatesRef.current, ...updates };

      if (saveTimeoutRef.current) {
        window.clearTimeout(saveTimeoutRef.current);
      }

      saveTimeoutRef.current = window.setTimeout(async () => {
        try {
          await updateConfig(pendingUpdatesRef.current!);
          pendingUpdatesRef.current = null;
        } catch (error) {
          logger.error('Failed to save OBS config:', error);
        }
      }, clientConfig.AUTO_SAVE_DELAY_MS);
    },
    [updateConfig],
  );

  // Field-level handlers.
  const handleHostBlur = useCallback(() => {
    if (config && host !== config.host) autoSave({ host });
  }, [host, config, autoSave]);

  const handlePortBlur = useCallback(() => {
    const portNum = parseInt(port, 10) || 4455;
    if (config && portNum !== config.port) autoSave({ port: portNum });
  }, [port, config, autoSave]);

  const handlePasswordBlur = useCallback(() => {
    if (config && password !== config.password) autoSave({ password });
  }, [password, config, autoSave]);

  const handleCopyPassword = useCallback(async () => {
    if (!password) return;
    try {
      await navigator.clipboard.writeText(password);
      toast.success(t('common.copied'));
    } catch {
      toast.error(t('common.error'));
    }
  }, [password, t]);

  // Immediate-save handlers (no debounce — avoid race conditions on toggles).
  const handleUseAuthChange = useCallback(
    async (checked: boolean) => {
      setUseAuth(checked);
      if (!checked) setPassword('');
      try {
        await updateConfig({ useAuth: checked });
      } catch (error) {
        logger.error('Failed to save useAuth:', error);
      }
    },
    [updateConfig],
  );

  const handleAutoConnectChange = useCallback(
    async (checked: boolean) => {
      setAutoConnect(checked);
      try {
        await updateConfig({ autoConnect: checked });
      } catch (error) {
        logger.error('Failed to save autoConnect:', error);
      }
    },
    [updateConfig],
  );

  const handleDirectionSelect = useCallback(
    (next: ObsIntegrationDirection) => {
      setDirection(next);
      updateConfig({ direction: next }).catch(logger.error);
    },
    [updateConfig],
  );

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

  const isConnected = connectionStatus === 'connected';

  if (!currentProfile) {
    return (
      <div className="flex items-center justify-center p-8 text-text-tertiary">
        {t('common.loadProfileFirst', 'Please load a profile first')}
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-3">
        <div className="p-2 rounded-lg bg-bg-elevated">
          <Radio className="w-5 h-5 text-primary" />
        </div>
        <div>
          <h2 className="text-lg font-semibold text-text-primary">{t('obs.title')}</h2>
          <p className="text-sm text-text-secondary">{t('obs.description')}</p>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <ObsConnectionForm
          isConnected={isConnected}
          host={host}
          setHost={setHost}
          onHostBlur={handleHostBlur}
          port={port}
          setPort={setPort}
          onPortBlur={handlePortBlur}
          useAuth={useAuth}
          onUseAuthChange={handleUseAuthChange}
          password={password}
          setPassword={setPassword}
          onPasswordBlur={handlePasswordBlur}
          showPassword={showPassword}
          setShowPassword={setShowPassword}
          onCopyPassword={handleCopyPassword}
          autoConnect={autoConnect}
          onAutoConnectChange={handleAutoConnectChange}
        />
        <ObsStatusCard
          connectionStatus={connectionStatus}
          streamStatus={streamStatus}
          errorMessage={errorMessage}
          obsVersion={obsVersion}
          websocketVersion={websocketVersion}
          isLoading={isLoading}
          onConnect={handleConnect}
          onDisconnect={handleDisconnect}
        />
      </div>

      <ObsDirectionSelector direction={direction} onSelect={handleDirectionSelect} />
    </div>
  );
}
