import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { emit, listen } from '@tauri-apps/api/event';

import { ConnectionError } from '@/components/ui/ConnectionError';
import {
  useProfileStore,
  subscribeProfileActivated,
  subscribeOAuthTokenExpired,
} from '@/stores/profileStore';
import { useLanguageStore } from '@/stores/languageStore';
import { useInitialize } from '@/hooks/useInitialize';
import { useStreamStats } from '@/hooks/useStreamStats';
import { useLogListener } from '@/hooks/useLogListener';
import { useChatListener } from '@/hooks/useChatListener';
import { useConnectionStatus } from '@/hooks/useConnectionStatus';
import { useBackendConnection } from '@/hooks/useBackendConnection';
import { useDataSync } from '@/hooks/useDataSync';
import { useObsEvents } from '@/hooks/useObsEvents';
import { useFollowerOnlyNotices } from '@/hooks/useFollowerOnlyNotices';
import { useOAuthCompletion } from '@/hooks/useOAuthCompletion';
import { useThemeStore, subscribeThemesUpdated } from '@/stores/themeStore';
import { hydrateClientConfig } from '@/lib/constants';
import { hydrateEncoderPresets, hydrateEncoderMetadata } from '@/lib/encoderPresets';
import { ChatOverlay } from '@/views/ChatOverlay';
import { checkAuth, getBackendBaseUrl, ServerReadyStatus, isTauri } from '@spiritstream/api-client';
import { initConnection } from '@spiritstream/api-client';
import { setupMainWindowCloseHandler } from '@/lib/chatWindow';
import { CHAT_OVERLAY_SYNC_EVENT, CHAT_OVERLAY_SYNC_REQUEST_EVENT } from '@/lib/chatEvents';
import { useChatStore } from '@/stores/chatStore';
import { logger } from '@/lib/logger';
import { ErrorBoundary } from '@/components/ErrorBoundary';
import { SinglePanelShell } from '@/SinglePanelShell';

const getWindowLabel = () => {
  // Check URL query parameter first (for browser popup mode)
  if (typeof window !== 'undefined') {
    const params = new URLSearchParams(window.location.search);
    if (params.get('overlay') === 'chat') {
      return 'chat-overlay';
    }
  }

  // Check Tauri window label (for desktop mode)
  try {
    return WebviewWindow.getCurrent().label;
  } catch {
    return 'main';
  }
};

/**
 * Main App component - handles server health checking before rendering main content.
 * This separation ensures that backend-dependent hooks only run after server is ready.
 */
function App() {
  const [windowLabel] = useState(getWindowLabel);

  if (windowLabel === 'chat-overlay') {
    return <ChatOverlayApp />;
  }

  return <MainApp />;
}

function ChatOverlayApp() {
  useChatListener();
  useThemeStore((state) => state.currentThemeId);

  return <ChatOverlay />;
}

function MainApp() {
  const { t } = useTranslation();

  type ServerStatus = 'checking' | 'unreachable' | 'not-ready' | 'ready';

  // Server health check state
  const [serverStatus, setServerStatus] = useState<ServerStatus>('checking');
  const [readyDetails, setReadyDetails] = useState<ServerReadyStatus | null>(null);
  const [isCheckingHealth, setIsCheckingHealth] = useState(false);

  useEffect(() => {
    const handleMessage = (event: MessageEvent) => {
      // Reject sync requests from other origins — without this an
      // iframe attacker on a different origin can request the full
      // chat history. The reply already targets `window.location.origin`,
      // but the request handler must also gate-keep on origin.
      if (event.origin !== window.location.origin) return;
      if (!event.data || event.data.type !== 'chat-overlay-sync-request') return;
      const messages = useChatStore.getState().messages;
      const target = event.source as Window | null;
      target?.postMessage(
        { type: 'chat-overlay-sync', messages },
        { targetOrigin: window.location.origin }
      );
    };

    window.addEventListener('message', handleMessage);
    return () => window.removeEventListener('message', handleMessage);
  }, []);

  useEffect(() => {
    if (!isTauri()) return;

    let cancelled = false;
    let unlisten: (() => void) | null = null;
    listen(CHAT_OVERLAY_SYNC_REQUEST_EVENT, () => {
      const messages = useChatStore.getState().messages;
      emit(CHAT_OVERLAY_SYNC_EVENT, { messages }).catch((error) => {
        logger.error('Failed to sync chat overlay:', error);
      });
    })
      .then((unsubscribe) => {
        // Unmounted while the registration was in flight — release it
        // immediately so the listener can't leak.
        if (cancelled) {
          unsubscribe();
          return;
        }
        unlisten = unsubscribe;
      })
      .catch((error) => {
        logger.error('Failed to register chat overlay sync listener:', error);
      });

    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, []);

  const formatReadyDetails = (details: ServerReadyStatus | null): string[] => {
    if (!details) return [];

    if (details.errors?.length) {
      return details.errors.map((item) => `${item.check}: ${item.error}`);
    }

    if (details.failed?.length) {
      return details.failed.map((item) => `${item} check failed`);
    }

    if (typeof details.status === 'number') {
      return [`HTTP ${details.status} from /api/v1/ready`];
    }

    if (details.lastError) {
      return [details.lastError];
    }

    return [];
  };

  // Wait for server readiness on mount via a single long-poll request.
  //
  // The backend's `GET /api/v1/ready` holds the connection (up to 25s)
  // until `ServerReadiness::ready` flips, then returns 200. If still
  // initializing past the timeout, it returns 503 + `Retry-After: 1`.
  //
  // ONE pattern across all deployments — Tauri desktop, Tauri mobile,
  // Docker, browser. No deployment-mode branching. No IPC. No polling
  // loop. No fallback chain. The forward-only architecture surfaces
  // exactly one source of truth: the server's readiness state.
  const probeReadiness = useCallback(async (signal: AbortSignal): Promise<void> => {
    setIsCheckingHealth(true);
    setServerStatus('checking');
    setReadyDetails(null);
    try {
      // L1: the readiness probe is the single intentional exception
      // to the "no direct fetch from a component" rule. The api-client
      // (`makeApiClient`) is configured against the server we're
      // probing — using it here would be chicken-and-egg. See
      // `packages/api-client/src/config.ts` (Server Readiness block).
      const resp = await fetch(`${getBackendBaseUrl()}/api/v1/ready`, {
        signal,
        credentials: 'include',
      });
      if (signal.aborted) return;
      if (resp.ok) {
        setServerStatus('ready');
        setReadyDetails(null);
      } else {
        const body = (await resp.json().catch(() => null)) as ServerReadyStatus | null;
        setServerStatus('not-ready');
        setReadyDetails({
          ready: false,
          status: resp.status,
          failed: body?.failed,
          errors: body?.errors,
          lastError: body?.lastError,
        });
      }
    } catch (err) {
      if (signal.aborted) return;
      setServerStatus('unreachable');
      logger.error('Readiness probe failed:', err);
    } finally {
      if (!signal.aborted) setIsCheckingHealth(false);
    }
  }, []);

  useEffect(() => {
    const controller = new AbortController();
    void probeReadiness(controller.signal);
    return () => {
      controller.abort();
    };
  }, [probeReadiness]);

  // Retry handler for the connection-error overlay. Uses the same
  // single-shot long-poll as the initial probe.
  const handleRetryConnection = async () => {
    const controller = new AbortController();
    await probeReadiness(controller.signal);
  };

  // Show connection error overlay if server is unreachable
  if (serverStatus === 'unreachable' || serverStatus === 'not-ready') {
    const details = serverStatus === 'not-ready' ? formatReadyDetails(readyDetails) : undefined;
    const title =
      serverStatus === 'not-ready'
        ? t('connection.notReadyTitle', { defaultValue: 'Backend not ready' })
        : undefined;
    const description =
      serverStatus === 'not-ready'
        ? t('connection.notReadyDescription', {
            defaultValue: 'The backend is running but failed its readiness checks.',
          })
        : undefined;
    const helpText =
      serverStatus === 'not-ready'
        ? t('connection.notReadyHelp', {
            defaultValue: 'Fix the issue(s) above or check the server logs, then retry.',
          })
        : undefined;

    return (
      <ConnectionError
        onRetry={handleRetryConnection}
        isRetrying={isCheckingHealth}
        title={title}
        description={description}
        helpText={helpText}
        details={details}
      />
    );
  }

  // Show loading state while checking server health
  if (serverStatus === 'checking') {
    return (
      <div className="fixed inset-0 flex items-center justify-center bg-bg-base">
        <div className="text-text-secondary">
          {t('common.loading', { defaultValue: 'Loading...' })}
        </div>
      </div>
    );
  }

  // Server is healthy - render the main app content
  // This component contains all the hooks that depend on backend connectivity.
  // Wrap the entire content tree in an ErrorBoundary so a
  // crashed view never leaves the user staring at a blank page.
  return (
    <ErrorBoundary>
      <AppContent />
    </ErrorBoundary>
  );
}

/**
 * Main app content - only rendered after server health is confirmed.
 * All backend-dependent hooks are safely contained here.
 */
function AppContent() {
  // Initialize backend connection (HTTP mode only)
  // IMPORTANT: This hook and others below only run after server is confirmed healthy
  useBackendConnection();

  // Hydrate server-tuned client constants (ranges, encoder presets, encoder
  // metadata). Runs once after the readiness gate passes — this is the
  // architectural reason these were moved out of main.tsx, where they raced
  // server startup and produced "could not connect" noise in the console.
  useEffect(() => {
    void hydrateClientConfig();
    void hydrateEncoderPresets();
    void hydrateEncoderMetadata();
  }, []);

  // Subscribe to backend WebSocket events. All `events.on(...)` calls are
  // gated through this hook (and only this hook) so that the WebSocket
  // opens AFTER the server-readiness gate passes. Subscribing at module-
  // import time produced the "WebSocket connection failed" cascade in
  // the boot console — every subscription now lives here instead.
  useEffect(() => {
    let unsubscribeThemes: (() => void) | null = null;
    let unsubscribeProfile: (() => void) | null = null;
    let unsubscribeOAuth: (() => void) | null = null;

    subscribeThemesUpdated()
      .then((unsub) => {
        unsubscribeThemes = unsub;
      })
      .catch((err) => logger.error('Failed to subscribe to themes_updated events:', err));

    subscribeProfileActivated()
      .then((unsub) => {
        unsubscribeProfile = unsub;
      })
      .catch((err) => logger.error('Failed to subscribe to profile_activated events:', err));

    subscribeOAuthTokenExpired()
      .then((unsub) => {
        unsubscribeOAuth = unsub;
      })
      .catch((err) => logger.error('Failed to subscribe to oauth_token_expired events:', err));

    return () => {
      if (unsubscribeThemes) unsubscribeThemes();
      if (unsubscribeProfile) unsubscribeProfile();
      if (unsubscribeOAuth) unsubscribeOAuth();
    };
  }, []);

  // Load the authoritative theme catalog from the backend. Runs after
  // the readiness gate, NOT during persist rehydration — the previous
  // architecture fired this at module-import time and raced server
  // startup. localStorage already supplied the user's chosen theme + its
  // tokens for first paint (via index.html inline script), so this load
  // is non-blocking: it only matters for Settings to enumerate themes
  // and for `themes_updated` to reconcile the cached themeId.
  useEffect(() => {
    // Fire-and-forget: the inline-tokens path already painted the UI,
    // so a refresh failure here only matters for Settings enumeration.
    useThemeStore
      .getState()
      .refreshThemes()
      .catch(() => {});
  }, []);

  // Initialize app - load profiles from backend
  useInitialize();

  // Listen to real-time stream stats from backend
  useStreamStats();

  // Capture logs throughout the app lifecycle
  useLogListener();

  // Listen to unified chat messages
  useChatListener();

  // Listen for backend connection status changes (HTTP mode only)
  useConnectionStatus();

  // Sync data when other clients make changes (HTTP mode only)
  useDataSync();

  // Listen for OBS WebSocket events and handle OBS -> SpiritStream triggering
  useObsEvents();

  // Surface follower-only applied/unsupported outcomes (toast + re-auth hint)
  useFollowerOnlyNotices();

  // Reflect completed OAuth sign-ins: reload the profile + success toast
  useOAuthCompletion();

  // Set up handler to close chat overlay when main window closes
  useEffect(() => setupMainWindowCloseHandler(), []);

  // Store hooks — only the profile-settings effect below needs these.
  // SinglePanelShell holds its own subscriptions for what it renders.
  const { initFromSettings: setLanguageFromProfile } = useLanguageStore();
  const { setTheme } = useThemeStore();
  const current = useProfileStore((s) => s.current);

  // Apply profile-specific theme and language when profile changes
  // NOTE: currentThemeId is intentionally NOT in deps — this effect should only
  // fire when the profile changes, not when the user switches themes via Settings.
  // Including it caused a snap-back loop: theme change → effect sees stale profile
  // themeId → resets theme → profile updates → effect fires again.
  useEffect(() => {
    const applyProfileSettings = async () => {
      if (!current?.settings) return;

      try {
        // Apply theme (profile-specific)
        const themeId = current.settings.themeId;
        const activeThemeId = useThemeStore.getState().currentThemeId;
        if (themeId && themeId !== activeThemeId) {
          await setTheme(themeId);
        }

        // Apply language (profile-specific)
        if (current.settings.language) {
          setLanguageFromProfile(current.settings.language);
        }
      } catch (error) {
        logger.error('Failed to apply profile settings:', error);
      }
    };

    applyProfileSettings();
  }, [current, setTheme, setLanguageFromProfile]);

  // Auth gate — SinglePanelShell renders the LoginModal using this state.
  const [loginModalOpen, setLoginModalOpen] = useState(false);

  // Handle authentication requirement events from backend
  useEffect(() => {
    const handleAuthRequired = () => {
      setLoginModalOpen(true);
    };

    // Listen for auth required events from backend (WebSocket auth failure)
    window.addEventListener('backend:auth-required', handleAuthRequired);

    return () => {
      window.removeEventListener('backend:auth-required', handleAuthRequired);
    };
  }, []);

  // Check auth status on mount (server is already confirmed healthy at this point)
  useEffect(() => {
    checkAuth().then((status) => {
      if (status.required && !status.authenticated) {
        setLoginModalOpen(true);
      }
    });
  }, []);

  // Handle successful login — reload to reinitialize all data with new auth.
  const handleLoginSuccess = useCallback(() => {
    setLoginModalOpen(false);
    initConnection();
    window.location.reload();
  }, []);

  return <SinglePanelShell loginModalOpen={loginModalOpen} onLoginSuccess={handleLoginSuccess} />;
}

export default App;
