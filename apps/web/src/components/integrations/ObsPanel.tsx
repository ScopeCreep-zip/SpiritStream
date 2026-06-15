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
import type { ObsIntegrationDirection, ObsSettings } from '@spiritstream/types';

/**
 * OBS integration orchestrator. Owns the form state, debounced auto-save,
 * unmount-flush, and store wiring. Renders three focused cards (connection
 * form, status, direction selector) — each is its own component under
 * `components/obs/`.
 */
export function ObsPanel() {
  const { t } = useTranslation();
  const currentProfile = useProfileStore((state) => state.current);
  const updateProfileSettings = useProfileStore((state) => state.updateProfileSettings);
  // OBS settings come from the active profile — the single source of truth.
  const obsSettings = currentProfile?.settings?.obs;
  const {
    connectionStatus,
    streamStatus,
    errorMessage,
    obsVersion,
    websocketVersion,
    showPassword,
    setShowPassword,
    loadState,
    connect,
    disconnect,
  } = useObsStore();

  // Local form state — mirrors the profile's OBS settings while editing; saved
  // back to the profile via autoSave / immediate update.
  const [host, setHost] = useState('localhost');
  const [port, setPort] = useState('4455');
  const [password, setPassword] = useState('');
  const [useAuth, setUseAuth] = useState(false);
  const [direction, setDirection] = useState<ObsIntegrationDirection>('disabled');
  const [autoConnect, setAutoConnect] = useState(false);

  // Debounced save infrastructure.
  const saveTimeoutRef = useRef<number | null>(null);
  const pendingUpdatesRef = useRef<Partial<ObsSettings> | null>(null);

  // Persist OBS settings to the active profile (the single source of truth).
  // Toggles/blurs send only the changed field; the merge keeps every other
  // value — including the real password — so a partial edit never blanks it.
  // The backend re-syncs its handler from the saved profile (apply_profile_obs).
  const saveObs = useCallback(
    async (updates: Partial<ObsSettings>) => {
      const cur = useProfileStore.getState().current?.settings?.obs;
      if (!cur) return;
      await updateProfileSettings({ obs: { ...cur, ...updates } });
    },
    [updateProfileSettings]
  );

  // Load initial OBS connection state (not settings — those come from profile).
  useEffect(() => {
    loadState();
  }, [loadState]);

  // Sync the form from the active profile's OBS settings.
  useEffect(() => {
    if (obsSettings) {
      setHost(obsSettings.host || 'localhost');
      setPort(String(obsSettings.port || 4455));
      setPassword(obsSettings.password || '');
      setUseAuth(obsSettings.useAuth);
      setDirection(obsSettings.direction);
      setAutoConnect(obsSettings.autoConnect);
    }
  }, [obsSettings]);

  // Flush pending saves on unmount.
  useEffect(() => {
    return () => {
      if (saveTimeoutRef.current) {
        window.clearTimeout(saveTimeoutRef.current);
      }
      if (pendingUpdatesRef.current) {
        saveObs(pendingUpdatesRef.current).catch((error) => {
          logger.error('Failed to flush OBS settings on unmount:', error);
        });
        pendingUpdatesRef.current = null;
      }
    };
  }, [saveObs]);

  // Auto-save with debounce.
  const autoSave = useCallback(
    (updates: Partial<ObsSettings>) => {
      pendingUpdatesRef.current = { ...pendingUpdatesRef.current, ...updates };

      if (saveTimeoutRef.current) {
        window.clearTimeout(saveTimeoutRef.current);
      }

      saveTimeoutRef.current = window.setTimeout(async () => {
        try {
          await saveObs(pendingUpdatesRef.current!);
          pendingUpdatesRef.current = null;
        } catch (error) {
          logger.error('Failed to save OBS settings:', error);
        }
      }, clientConfig.AUTO_SAVE_DELAY_MS);
    },
    [saveObs]
  );

  // Field-level handlers.
  const handleHostBlur = useCallback(() => {
    if (obsSettings && host !== obsSettings.host) autoSave({ host });
  }, [host, obsSettings, autoSave]);

  const handlePortBlur = useCallback(() => {
    const portNum = parseInt(port, 10) || 4455;
    if (obsSettings && portNum !== obsSettings.port) autoSave({ port: portNum });
  }, [port, obsSettings, autoSave]);

  const handlePasswordBlur = useCallback(() => {
    if (obsSettings && password !== obsSettings.password) autoSave({ password });
  }, [password, obsSettings, autoSave]);

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
      const updates: Partial<ObsSettings> = { useAuth: checked };
      if (!checked) {
        setPassword('');
        updates.password = '';
      }
      try {
        await saveObs(updates);
      } catch (error) {
        logger.error('Failed to save useAuth:', error);
      }
    },
    [saveObs]
  );

  const handleAutoConnectChange = useCallback(
    async (checked: boolean) => {
      setAutoConnect(checked);
      try {
        await saveObs({ autoConnect: checked });
      } catch (error) {
        logger.error('Failed to save autoConnect:', error);
      }
    },
    [saveObs]
  );

  const handleDirectionSelect = useCallback(
    (next: ObsIntegrationDirection) => {
      setDirection(next);
      saveObs({ direction: next }).catch(logger.error);
    },
    [saveObs]
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
          isLoading={connectionStatus === 'connecting'}
          onConnect={handleConnect}
          onDisconnect={handleDisconnect}
        />
      </div>

      <ObsDirectionSelector direction={direction} onSelect={handleDirectionSelect} />
    </div>
  );
}
