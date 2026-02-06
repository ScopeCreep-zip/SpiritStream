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
  MessageSquare,
  Plug,
} from 'lucide-react';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';

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
import { useProfileStore } from '@/stores/profileStore';
import { useStreamStore } from '@/stores/streamStore';
import { useLanguageStore } from '@/stores/languageStore';
import { useInitialize } from '@/hooks/useInitialize';
import { useStreamStats } from '@/hooks/useStreamStats';
import { useLogListener } from '@/hooks/useLogListener';
import { useChatListener } from '@/hooks/useChatListener';
import { useConnectionStatus } from '@/hooks/useConnectionStatus';
import { useBackendConnection } from '@/hooks/useBackendConnection';
import { useDataSync } from '@/hooks/useDataSync';
import { useObsEvents } from '@/hooks/useObsEvents';
import { validateStreamConfig, displayValidationIssues } from '@/lib/streamValidation';
import { toast } from '@/hooks/useToast';
import { useThemeStore } from '@/stores/themeStore';
import { ChatOverlay } from '@/views/ChatOverlay';
import { checkAuth, checkServerHealth, checkServerReadyDetailed, ServerReadyStatus } from '@/lib/backend/env';
import { initConnection } from '@/lib/backend/httpEvents';
import { api } from '@/lib/backend';

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
  Chat,
  Integrations,
} from '@/views';

export type View =
  | 'dashboard'
  | 'profiles'
  | 'streams'
  | 'encoder'
  | 'outputs'
  | 'targets'
  | 'chat'
  | 'logs'
  | 'settings'
  | 'integrations';

// View meta is now handled via translations using keys like header.dashboard.title
const getWindowLabel = () => {
  try {
    return WebviewWindow.getCurrent().label;
  } catch (error) {
    console.warn('Failed to read current window label:', error);
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

  const formatReadyDetails = (details: ServerReadyStatus | null): string[] => {
    if (!details) return [];

    if (details.errors?.length) {
      return details.errors.map((item) => `${item.check}: ${item.error}`);
    }

    if (details.failed?.length) {
      return details.failed.map((item) => `${item} check failed`);
    }

    if (typeof details.status === 'number') {
      return [`HTTP ${details.status} from /ready`];
    }

    if (details.lastError) {
      return [details.lastError];
    }

    return [];
  };

  // Check server health on mount
  useEffect(() => {
    let cancelled = false;

    const checkHealth = async () => {
      setIsCheckingHealth(true);
      setServerStatus('checking');
      setReadyDetails(null);

      // Give the server time to start (especially in Tauri where launcher is spawning it)
      const healthy = await checkServerHealth(10, 500);
      if (cancelled) return;

      if (!healthy) {
        setServerStatus('unreachable');
        setIsCheckingHealth(false);
        return;
      }

      const ready = await checkServerReadyDetailed(15, 400);
      if (cancelled) return;

      if (ready.ready) {
        setServerStatus('ready');
        setReadyDetails(null);
      } else {
        setServerStatus('not-ready');
        setReadyDetails(ready);
      }
      setIsCheckingHealth(false);
    };

    checkHealth();

    return () => {
      cancelled = true;
    };
  }, []);

  // Retry handler for connection error
  const handleRetryConnection = async () => {
    setIsCheckingHealth(true);
    setServerStatus('checking');
    setReadyDetails(null);

    const healthy = await checkServerHealth(15, 500);
    if (healthy) {
      const ready = await checkServerReadyDetailed(15, 400);
      if (ready.ready) {
        setServerStatus('ready');
        setReadyDetails(null);
      } else {
        setServerStatus('not-ready');
        setReadyDetails(ready);
      }
    } else {
      setServerStatus('unreachable');
    }
    setIsCheckingHealth(false);
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

  // Listen to unified chat messages
  useChatListener();

  // Listen for backend connection status changes (HTTP mode only)
  useConnectionStatus();

  // Sync data when other clients make changes (HTTP mode only)
  useDataSync();

  // Listen for OBS WebSocket events and handle OBS -> SpiritStream triggering
  useObsEvents();

  // Store hooks
  const { setLanguage } = useLanguageStore();
  const { currentThemeId, setTheme } = useThemeStore();

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

  // Apply profile-specific theme and language when profile changes
  useEffect(() => {
    const applyProfileSettings = async () => {
      if (!current) return;

      try {
        // Get global settings as fallback
        const globalSettings = await api.settings.get();

        // Apply theme (profile-specific or global fallback)
        const effectiveTheme = current.theme || globalSettings.themeId;
        if (effectiveTheme && effectiveTheme !== currentThemeId) {
          await setTheme(effectiveTheme);
        }

        // Apply language (profile-specific or global fallback)
        const effectiveLanguage = current.language || globalSettings.language;
        if (effectiveLanguage) {
          setLanguage(effectiveLanguage as any);
        }
      } catch (error) {
        console.error('Failed to apply profile settings:', error);
      }
    };

    applyProfileSettings();
  }, [current, currentThemeId, setTheme, setLanguage]);

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
      // Build incoming URL from structured input
      const incomingUrl = `rtmp://${current.input.bindAddress}:${current.input.port}/${current.input.application}`;
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
      case 'streams':
        return <StreamManager onNavigate={handleNavigate} />;
      case 'encoder':
        return <EncoderSettings />;
      case 'outputs':
        return <OutputGroups />;
      case 'targets':
        return <StreamTargets />;
      case 'chat':
        return <Chat />;
      case 'logs':
        return <Logs />;
      case 'settings':
        return <Settings />;
      case 'integrations':
        return <Integrations />;
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
              icon={<MessageSquare className="w-5 h-5" />}
              label={t('nav.chat', { defaultValue: 'Chat' })}
              active={currentView === 'chat'}
              onClick={() => setCurrentView('chat')}
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
              icon={<Plug className="w-5 h-5" />}
              label={t('nav.integrations')}
              active={currentView === 'integrations'}
              onClick={() => setCurrentView('integrations')}
            />
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
