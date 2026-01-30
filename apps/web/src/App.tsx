import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import {
  LayoutDashboard,
  Users,
  Radio,
  Settings2,
  Share2,
  Target,
  FileText,
  Cog,
  Play,
  Square,
  Video,
} from 'lucide-react';

import { AppShell } from '@/components/layout/AppShell';
import { Sidebar, SidebarHeader, SidebarNav, SidebarFooter } from '@/components/layout/Sidebar';
import { Logo } from '@/components/layout/Logo';
import { MainContent, ContentArea } from '@/components/layout/MainContent';
import { Header } from '@/components/layout/Header';
import { NavSection } from '@/components/navigation/NavSection';
import { NavItem } from '@/components/navigation/NavItem';
import { Button } from '@/components/ui/Button';
import { ToastContainer } from '@/components/ui/Toast';
import { ConnectionStatus } from '@/components/ui/ConnectionStatus';
import { ConnectionError } from '@/components/ui/ConnectionError';
import { ProfileModal, TargetModal, OutputGroupModal, LoginModal } from '@/components/modals';
import { PasswordModal } from '@/components/modals/PasswordModal';
import { WebRTCConnectionManager } from '@/components/WebRTCConnectionManager';
import { useProfileStore } from '@/stores/profileStore';
import { useStreamStore } from '@/stores/streamStore';
import { useInitialize } from '@/hooks/useInitialize';
import { useStreamStats } from '@/hooks/useStreamStats';
import { useLogListener } from '@/hooks/useLogListener';
import { useConnectionStatus } from '@/hooks/useConnectionStatus';
import { useBackendConnection } from '@/hooks/useBackendConnection';
import { useDataSync } from '@/hooks/useDataSync';
import { validateStreamConfig, displayValidationIssues } from '@/lib/streamValidation';
import { getIncomingUrl } from '@/types/profile';
import { toast } from '@/hooks/useToast';
import { useThemeStore } from '@/stores/themeStore';
import { isTauri, checkAuth, checkServerHealth, checkServerReady } from '@/lib/backend/env';
import { initConnection } from '@/lib/backend/httpEvents';

// Import all views
import {
  Dashboard,
  Profiles,
  StreamManager,
  EncoderSettings,
  OutputGroups,
  StreamTargets,
  Logs,
  Settings,
} from '@/views';
import { Stream } from '@/views/Stream';

export type View =
  | 'dashboard'
  | 'profiles'
  | 'stream'
  | 'streams'
  | 'encoder'
  | 'outputs'
  | 'targets'
  | 'logs'
  | 'settings';

// View meta is now handled via translations using keys like header.dashboard.title

/**
 * Main App component - handles server health checking before rendering main content.
 * This separation ensures that backend-dependent hooks only run after server is ready.
 */
function App() {
  const { t } = useTranslation();

  // Server health check state
  const [serverHealthy, setServerHealthy] = useState<boolean | null>(null);
  const [isCheckingHealth, setIsCheckingHealth] = useState(false);

  // Check server health on mount
  // In Tauri mode: Listen for 'server-ready' event from Rust launcher (no polling errors)
  // In HTTP mode: Fall back to polling /health endpoint (for Docker/remote browser access)
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    const initHealthCheck = async () => {
      setIsCheckingHealth(true);

      // In Tauri, wait for the Rust launcher to signal server is ready
      // This avoids redundant polling and console errors during startup
      if (isTauri()) {
        try {
          const { listen } = await import('@tauri-apps/api/event');

          let resolved = false;
          const markReady = () => {
            if (resolved || cancelled) return;
            resolved = true;
            setServerHealthy(true);
            setIsCheckingHealth(false);
          };

          const markFailed = (error?: string) => {
            if (resolved || cancelled) return;
            resolved = true;
            console.error('Server startup failed:', error);
            setServerHealthy(false);
            setIsCheckingHealth(false);
          };

          // Set up listener for server-ready event from Rust launcher
          unlisten = await listen('server-ready', markReady);

          // Also listen for server-error event
          await listen('server-error', (event) => markFailed(event.payload as string));

          // Race condition fix: The server-ready event might have been emitted
          // before our listener was set up. Wait a bit longer then check.
          // Use 500ms delay to give server time to start, avoiding console errors.
          setTimeout(async () => {
            if (resolved || cancelled) return;
            try {
              const healthy = await checkServerHealth(1, 0);
              if (healthy && !resolved && !cancelled) {
                const ready = await checkServerReady(1, 0);
                if (ready && !resolved && !cancelled) {
                  console.log('[App] Server already ready (caught race condition)');
                  markReady();
                }
              }
            } catch {
              // Ignore errors - we still have the event listener as backup
            }
          }, 500); // Wait 500ms to avoid console errors from early failed requests

          // Fallback: If event doesn't arrive within 20 seconds, something is wrong.
          // This handles edge cases where event system fails entirely.
          setTimeout(() => {
            if (resolved || cancelled) return;
            markFailed('Server startup timeout - no ready signal received');
          }, 20000);
        } catch (error) {
          console.warn('Failed to set up Tauri event listener, falling back to polling:', error);
          // Fall through to HTTP polling below
          await pollHealthCheck();
        }
      } else {
        // Not in Tauri - use HTTP polling (Docker, remote browser access)
        await pollHealthCheck();
      }
    };

    const pollHealthCheck = async () => {
      const healthy = await checkServerHealth(10, 500);
      if (cancelled) return;

      if (!healthy) {
        setServerHealthy(false);
        setIsCheckingHealth(false);
        return;
      }

      const ready = await checkServerReady(15, 400);
      if (cancelled) return;

      setServerHealthy(ready);
      setIsCheckingHealth(false);
    };

    initHealthCheck();

    return () => {
      cancelled = true;
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  // Retry handler for connection error
  const handleRetryConnection = async () => {
    setIsCheckingHealth(true);
    setServerHealthy(null); // Reset to loading state

    const healthy = await checkServerHealth(15, 500);
    if (healthy) {
      const ready = await checkServerReady(15, 400);
      setServerHealthy(ready);
    } else {
      setServerHealthy(false);
    }
    setIsCheckingHealth(false);
  };

  // Show connection error overlay if server is unreachable
  if (serverHealthy === false) {
    return <ConnectionError onRetry={handleRetryConnection} isRetrying={isCheckingHealth} />;
  }

  // Show loading state while checking server health
  if (serverHealthy === null) {
    return (
      <div className="fixed inset-0 flex items-center justify-center bg-[var(--bg-base)]">
        <div className="text-[var(--text-secondary)]">{t('common.loading', { defaultValue: 'Loading...' })}</div>
      </div>
    );
  }

  // Server is healthy - render the main app content
  // This component contains all the hooks that depend on backend connectivity
  return <AppContent />;
}

/**
 * Main app content - only rendered after server health is confirmed.
 * All backend-dependent hooks are safely contained here.
 */
function AppContent() {
  const { t } = useTranslation();

  // Initialize backend connection (HTTP mode only)
  // IMPORTANT: This hook and others below only run after server is confirmed healthy
  useBackendConnection();

  // Initialize app - load profiles from backend
  useInitialize();

  // Listen to real-time stream stats from backend
  useStreamStats();

  // Capture logs throughout the app lifecycle
  useLogListener();

  // Listen for backend connection status changes (HTTP mode only)
  useConnectionStatus();

  // Sync data when other clients make changes (HTTP mode only)
  useDataSync();

  // Initialize theme store on app startup
  useThemeStore((state) => state.currentThemeId);

  const [currentView, setCurrentView] = useState<View>('dashboard');
  const {
    current,
    profiles,
    pendingPasswordProfile,
    passwordError,
    submitPassword,
    cancelPasswordPrompt,
  } = useProfileStore();
  const { isStreaming, startAllGroups, stopAllGroups } = useStreamStore();

  // Modal state
  const [profileModalOpen, setProfileModalOpen] = useState(false);
  const [targetModalOpen, setTargetModalOpen] = useState(false);
  const [outputGroupModalOpen, setOutputGroupModalOpen] = useState(false);
  const [loginModalOpen, setLoginModalOpen] = useState(false);

  // Streaming validation state
  const [isValidating, setIsValidating] = useState(false);

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

  // Handle successful login
  const handleLoginSuccess = () => {
    setLoginModalOpen(false);
    // Reinitialize connection after successful login
    initConnection();
    // Reload to reinitialize all data with new auth
    window.location.reload();
  };

  // Get title and description from translations
  const title = t(`header.${currentView}.title`);
  const description = t(`header.${currentView}.description`);

  // Count profiles for badge from store
  const profileCount = profiles.length;

  // Get first output group ID for target modal (when adding from header)
  const firstGroupId = current?.outputGroups[0]?.id || '';

  const handleStartStreaming = async () => {
    if (!current) return;

    setIsValidating(true);

    try {
      // Run comprehensive validation (including FFmpeg check)
      // Note: Header button validates ALL targets since we don't have per-target toggles here
      const result = await validateStreamConfig(current, {
        checkFfmpeg: true,
        checkEnabledTargetsOnly: false,
      });

      if (!result.valid) {
        displayValidationIssues(result.issues, toast);
        return;
      }

      // Validation passed, start streaming
      // Get incoming URL from profile (supports both legacy and new source formats)
      const incomingUrl = getIncomingUrl(current);
      if (!incomingUrl) {
        toast.error('No RTMP input configured');
        return;
      }
      await startAllGroups(current.outputGroups, incomingUrl);
      toast.success(t('toast.streamStarted'));
    } catch (err) {
      console.error('[App] startAllGroups failed:', err);
      toast.error(
        t('toast.startFailed', {
          error: err instanceof Error ? err.message : String(err),
        })
      );
    } finally {
      setIsValidating(false);
    }
  };

  const handleStopStreaming = async () => {
    await stopAllGroups();
  };

  // Navigation handler to pass to views
  const handleNavigate = (view: View) => {
    setCurrentView(view);
  };

  const renderView = () => {
    switch (currentView) {
      case 'dashboard':
        return (
          <Dashboard
            onNavigate={handleNavigate}
            onOpenProfileModal={() => setProfileModalOpen(true)}
            onOpenTargetModal={() => setTargetModalOpen(true)}
          />
        );
      case 'profiles':
        return <Profiles />;
      case 'stream':
        return <Stream />;
      case 'streams':
        return <StreamManager onNavigate={handleNavigate} />;
      case 'encoder':
        return <EncoderSettings />;
      case 'outputs':
        return <OutputGroups />;
      case 'targets':
        return <StreamTargets />;
      case 'logs':
        return <Logs />;
      case 'settings':
        return <Settings />;
      default:
        return (
          <Dashboard
            onNavigate={handleNavigate}
            onOpenProfileModal={() => setProfileModalOpen(true)}
            onOpenTargetModal={() => setTargetModalOpen(true)}
          />
        );
    }
  };

  // Render header actions based on current view
  const renderHeaderActions = () => {
    switch (currentView) {
      case 'dashboard':
      case 'streams':
        return isStreaming ? (
          <Button variant="destructive" onClick={handleStopStreaming}>
            <Square className="w-4 h-4" />
            {t('streams.stopStreaming')}
          </Button>
        ) : (
          <Button onClick={handleStartStreaming} disabled={isValidating || !current}>
            <Play className="w-4 h-4" />
            {isValidating ? t('streams.validating') : t('streams.startStreaming')}
          </Button>
        );
      case 'profiles':
        return (
          <Button onClick={() => setProfileModalOpen(true)}>{t('profiles.newProfile')}</Button>
        );
      case 'targets':
        return (
          <Button
            onClick={() => setTargetModalOpen(true)}
            disabled={!current || current.outputGroups.length === 0}
          >
            {t('targets.addTarget')}
          </Button>
        );
      case 'outputs':
        return (
          <Button onClick={() => setOutputGroupModalOpen(true)} disabled={!current}>
            {t('outputs.newOutputGroup')}
          </Button>
        );
      default:
        return null;
    }
  };

  return (
    <AppShell>
      {/* Persistent WebRTC connection manager - syncs connections with profile sources */}
      <WebRTCConnectionManager />

      <Sidebar>
        <SidebarHeader>
          <Logo />
        </SidebarHeader>
        <SidebarNav>
          <NavSection title={t('nav.main')}>
            <NavItem
              icon={<LayoutDashboard className="w-5 h-5" />}
              label={t('nav.dashboard')}
              active={currentView === 'dashboard'}
              onClick={() => setCurrentView('dashboard')}
            />
            <NavItem
              icon={<Users className="w-5 h-5" />}
              label={t('nav.profiles')}
              active={currentView === 'profiles'}
              onClick={() => setCurrentView('profiles')}
              badge={profileCount}
            />
            <NavItem
              icon={<Radio className="w-5 h-5" />}
              label={t('nav.streamManager')}
              active={currentView === 'streams'}
              onClick={() => setCurrentView('streams')}
            />
            <NavItem
              icon={<Video className="w-5 h-5" />}
              label="Stream Studio"
              active={currentView === 'stream'}
              onClick={() => setCurrentView('stream')}
            />
          </NavSection>
          <NavSection title={t('nav.configuration')}>
            <NavItem
              icon={<Settings2 className="w-5 h-5" />}
              label={t('nav.encoderSettings')}
              active={currentView === 'encoder'}
              onClick={() => setCurrentView('encoder')}
            />
            <NavItem
              icon={<Share2 className="w-5 h-5" />}
              label={t('nav.outputGroups')}
              active={currentView === 'outputs'}
              onClick={() => setCurrentView('outputs')}
            />
            <NavItem
              icon={<Target className="w-5 h-5" />}
              label={t('nav.streamTargets')}
              active={currentView === 'targets'}
              onClick={() => setCurrentView('targets')}
            />
          </NavSection>
          <NavSection title={t('nav.system')}>
            <NavItem
              icon={<FileText className="w-5 h-5" />}
              label={t('nav.logs')}
              active={currentView === 'logs'}
              onClick={() => setCurrentView('logs')}
            />
            <NavItem
              icon={<Cog className="w-5 h-5" />}
              label={t('nav.settings')}
              active={currentView === 'settings'}
              onClick={() => setCurrentView('settings')}
            />
          </NavSection>
        </SidebarNav>
        <SidebarFooter>
          <ConnectionStatus />
        </SidebarFooter>
      </Sidebar>

      <MainContent>
        <Header title={title} description={description}>
          {renderHeaderActions()}
        </Header>

        <ContentArea>{renderView()}</ContentArea>
      </MainContent>

      {/* Modals */}
      <ProfileModal
        open={profileModalOpen}
        onClose={() => setProfileModalOpen(false)}
        mode="create"
      />

      <TargetModal
        open={targetModalOpen}
        onClose={() => setTargetModalOpen(false)}
        mode="create"
        groupId={firstGroupId}
      />

      <OutputGroupModal
        open={outputGroupModalOpen}
        onClose={() => setOutputGroupModalOpen(false)}
        mode="create"
      />

      <PasswordModal
        open={!!pendingPasswordProfile}
        onClose={cancelPasswordPrompt}
        onSubmit={submitPassword}
        mode="decrypt"
        profileName={pendingPasswordProfile || undefined}
        error={passwordError || undefined}
      />

      <LoginModal
        open={loginModalOpen}
        onSuccess={handleLoginSuccess}
      />

      {/* Toast notifications */}
      <ToastContainer />
    </AppShell>
  );
}

export default App;
