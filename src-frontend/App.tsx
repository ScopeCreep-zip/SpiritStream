import { useState, useCallback } from 'react';
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

interface ViewMeta {
  title: string;
  description: string;
}

const viewMeta: Record<View, ViewMeta> = {
  dashboard: {
    title: 'Dashboard',
    description: 'Overview of your streaming setup',
  },
  profiles: {
    title: 'Streaming Profiles',
    description: 'Manage your saved streaming configurations',
  },
  streams: {
    title: 'Stream Manager',
    description: 'Control your live streaming operations',
  },
  encoder: {
    title: 'Encoder Settings',
    description: 'Configure video and audio encoding parameters',
  },
  outputs: {
    title: 'Output Groups',
    description: 'Manage output configurations and encoding presets',
  },
  targets: {
    title: 'Stream Targets',
    description: 'Configure streaming destinations and platforms',
  },
  logs: {
    title: 'Application Logs',
    description: 'View real-time application events and debug information',
  },
  settings: {
    title: 'Settings',
    description: 'Configure application preferences',
  },
};

function App() {
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

  const { title, description } = viewMeta[currentView];

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
      toast.success('Streaming started');
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
            Stop Streaming
          </Button>
        ) : (
          <Button onClick={handleStartStreaming} disabled={isValidating || !current}>
            <Play className="w-4 h-4" />
            {isValidating ? 'Validating...' : 'Start Streaming'}
          </Button>
        );
      case 'profiles':
        return (
          <Button onClick={() => setProfileModalOpen(true)}>
            New Profile
          </Button>
        );
      case 'targets':
        return (
          <Button
            onClick={() => setTargetModalOpen(true)}
            disabled={!current || current.outputGroups.length === 0}
          >
            Add Target
          </Button>
        );
      case 'outputs':
        return (
          <Button
            onClick={() => setOutputGroupModalOpen(true)}
            disabled={!current}
          >
            New Output Group
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
          <NavSection title="Main">
            <NavItem
              icon={<LayoutDashboard className="w-5 h-5" />}
              label="Dashboard"
              active={currentView === 'dashboard'}
              onClick={() => setCurrentView('dashboard')}
            />
            <NavItem
              icon={<Users className="w-5 h-5" />}
              label="Profiles"
              active={currentView === 'profiles'}
              onClick={() => setCurrentView('profiles')}
              badge={profileCount}
            />
            <NavItem
              icon={<Radio className="w-5 h-5" />}
              label="Stream Manager"
              active={currentView === 'streams'}
              onClick={() => setCurrentView('streams')}
            />
          </NavSection>
          <NavSection title="Configuration">
            <NavItem
              icon={<Settings2 className="w-5 h-5" />}
              label="Encoder Settings"
              active={currentView === 'encoder'}
              onClick={() => setCurrentView('encoder')}
            />
            <NavItem
              icon={<Share2 className="w-5 h-5" />}
              label="Output Groups"
              active={currentView === 'outputs'}
              onClick={() => setCurrentView('outputs')}
            />
            <NavItem
              icon={<Target className="w-5 h-5" />}
              label="Stream Targets"
              active={currentView === 'targets'}
              onClick={() => setCurrentView('targets')}
            />
          </NavSection>
          <NavSection title="System">
            <NavItem
              icon={<FileText className="w-5 h-5" />}
              label="Logs"
              active={currentView === 'logs'}
              onClick={() => setCurrentView('logs')}
            />
            <NavItem
              icon={<Cog className="w-5 h-5" />}
              label="Settings"
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
