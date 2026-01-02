import { useState } from 'react';
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
import { useProfileStore } from '@/stores/profileStore';
import { useStreamStore } from '@/stores/streamStore';

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

type View = 'dashboard' | 'profiles' | 'streams' | 'encoder' | 'outputs' | 'targets' | 'logs' | 'settings';

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
  const [currentView, setCurrentView] = useState<View>('dashboard');
  const { current, profiles } = useProfileStore();
  const { isStreaming, setIsStreaming, setActiveGroup } = useStreamStore();

  const { title, description } = viewMeta[currentView];

  // Count profiles for badge from store
  const profileCount = profiles.length;

  const handleStartStreaming = () => {
    if (!current) return;
    setIsStreaming(true);
    current.outputGroups.forEach(group => {
      setActiveGroup(group.id, true);
    });
    // TODO: Call Tauri to actually start streams
  };

  const handleStopStreaming = () => {
    if (!current) return;
    setIsStreaming(false);
    current.outputGroups.forEach(group => {
      setActiveGroup(group.id, false);
    });
    // TODO: Call Tauri to actually stop streams
  };

  const renderView = () => {
    switch (currentView) {
      case 'dashboard':
        return <Dashboard />;
      case 'profiles':
        return <Profiles />;
      case 'streams':
        return <StreamManager />;
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
        return <Dashboard />;
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
          <Button onClick={handleStartStreaming}>
            <Play className="w-4 h-4" />
            Start Streaming
          </Button>
        );
      case 'profiles':
        return (
          <Button onClick={() => alert('Create profile modal not yet implemented')}>
            New Profile
          </Button>
        );
      case 'targets':
        return (
          <Button onClick={() => alert('Add target modal not yet implemented')}>
            Add Target
          </Button>
        );
      case 'outputs':
        return (
          <Button onClick={() => alert('Create output group modal not yet implemented')}>
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
    </AppShell>
  );
}

export default App;
