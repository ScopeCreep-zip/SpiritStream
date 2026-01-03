import { useState } from 'react';
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
} from 'lucide-react';

import { AppShell } from '@/components/layout/AppShell';
import { Sidebar, SidebarHeader, SidebarNav, SidebarFooter } from '@/components/layout/Sidebar';
import { Logo } from '@/components/layout/Logo';
import { MainContent, ContentArea } from '@/components/layout/MainContent';
import { Header } from '@/components/layout/Header';
import { NavSection } from '@/components/navigation/NavSection';
import { NavItem } from '@/components/navigation/NavItem';
import { ThemeToggle } from '@/components/ui/ThemeToggle';
import { Button } from '@/components/ui/Button';
import { ToastContainer } from '@/components/ui/Toast';
import { ProfileModal, TargetModal, OutputGroupModal } from '@/components/modals';
import { PasswordModal } from '@/components/modals/PasswordModal';
import { useProfileStore } from '@/stores/profileStore';
import { useStreamStore } from '@/stores/streamStore';
import { useInitialize } from '@/hooks/useInitialize';
import { useStreamStats } from '@/hooks/useStreamStats';
import { validateStreamConfig, displayValidationIssues } from '@/lib/streamValidation';
import { toast } from '@/hooks/useToast';

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

export type View = 'dashboard' | 'profiles' | 'streams' | 'encoder' | 'outputs' | 'targets' | 'logs' | 'settings';

// View meta is now handled via translations using keys like header.dashboard.title

function App() {
  const { t } = useTranslation();

  // Initialize app - load profiles from backend
  useInitialize();

  // Listen to real-time stream stats from backend
  useStreamStats();

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

  // Streaming validation state
  const [isValidating, setIsValidating] = useState(false);

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
      await startAllGroups(current.outputGroups, current.incomingUrl);
      toast.success(t('toast.streamStarted'));
    } catch (err) {
      console.error('[App] startAllGroups failed:', err);
      toast.error(`Failed to start streaming: ${err instanceof Error ? err.message : String(err)}`);
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
          <Button onClick={() => setProfileModalOpen(true)}>
            {t('profiles.newProfile')}
          </Button>
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
          <Button
            onClick={() => setOutputGroupModalOpen(true)}
            disabled={!current}
          >
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
          <ThemeToggle />
        </SidebarFooter>
      </Sidebar>

      <MainContent>
        <Header
          title={title}
          description={description}
        >
          {renderHeaderActions()}
        </Header>

        <ContentArea>
          {renderView()}
        </ContentArea>
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

      {/* Toast notifications */}
      <ToastContainer />
    </AppShell>
  );
}

export default App;
