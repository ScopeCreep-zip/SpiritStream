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
import { StatsRow } from '@/components/dashboard/StatsRow';
import { StatBox } from '@/components/dashboard/StatBox';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Grid } from '@/components/ui/Grid';
import { ProfileCard } from '@/components/dashboard/ProfileCard';
import { StreamCard } from '@/components/dashboard/StreamCard';
import { Activity, AlertTriangle, Clock, Monitor, Gauge, Target as TargetIcon } from 'lucide-react';

type View = 'dashboard' | 'profiles' | 'streams' | 'encoder' | 'outputs' | 'targets' | 'logs' | 'settings';

function App() {
  const [currentView, setCurrentView] = useState<View>('dashboard');

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
              badge={3}
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
          title="Dashboard"
          description="Overview of your streaming setup"
        >
          <Button>Start Streaming</Button>
        </Header>

        <ContentArea>
          <StatsRow>
            <StatBox
              icon={<Radio className="w-5 h-5" />}
              label="Active Streams"
              value={0}
              change="Ready to start"
            />
            <StatBox
              icon={<Activity className="w-5 h-5" />}
              label="Total Bitrate"
              value="0 kbps"
              change="No active streams"
            />
            <StatBox
              icon={<AlertTriangle className="w-5 h-5" />}
              label="Dropped Frames"
              value={0}
              change="No issues"
              changeType="positive"
            />
            <StatBox
              icon={<Clock className="w-5 h-5" />}
              label="Uptime"
              value="00:00:00"
              change="Not streaming"
            />
          </StatsRow>

          <Grid cols={2} className="mb-6">
            <Card>
              <CardHeader>
                <div>
                  <CardTitle>Active Profile</CardTitle>
                  <CardDescription>Currently selected streaming configuration</CardDescription>
                </div>
                <Button variant="ghost" size="sm">Change</Button>
              </CardHeader>
              <CardBody>
                <ProfileCard
                  name="Gaming Stream - High Quality"
                  meta={[
                    { icon: <Monitor className="w-4 h-4" />, label: '1080p60' },
                    { icon: <Gauge className="w-4 h-4" />, label: '6000 kbps' },
                    { icon: <TargetIcon className="w-4 h-4" />, label: '3 targets' },
                  ]}
                  active
                />
              </CardBody>
            </Card>

            <Card>
              <CardHeader>
                <div>
                  <CardTitle>Quick Actions</CardTitle>
                  <CardDescription>Common streaming operations</CardDescription>
                </div>
              </CardHeader>
              <CardBody>
                <Grid cols={2} gap="sm">
                  <Button variant="outline" className="justify-start">
                    New Profile
                  </Button>
                  <Button variant="outline" className="justify-start">
                    Import Profile
                  </Button>
                  <Button variant="outline" className="justify-start">
                    Add Target
                  </Button>
                  <Button variant="outline" className="justify-start">
                    Test Stream
                  </Button>
                </Grid>
              </CardBody>
            </Card>
          </Grid>

          <Card>
            <CardHeader>
              <div>
                <CardTitle>Stream Targets</CardTitle>
                <CardDescription>Connected streaming platforms</CardDescription>
              </div>
              <Button variant="ghost" size="sm">Manage</Button>
            </CardHeader>
            <CardBody>
              <Grid cols={3}>
                <StreamCard
                  platform="youtube"
                  name="YouTube Gaming"
                  status="offline"
                  stats={[
                    { label: 'Viewers', value: 0 },
                    { label: 'Bitrate', value: '--' },
                    { label: 'FPS', value: '--' },
                  ]}
                />
                <StreamCard
                  platform="twitch"
                  name="Twitch"
                  status="offline"
                  stats={[
                    { label: 'Viewers', value: 0 },
                    { label: 'Bitrate', value: '--' },
                    { label: 'FPS', value: '--' },
                  ]}
                />
                <StreamCard
                  platform="kick"
                  name="Kick"
                  status="offline"
                  stats={[
                    { label: 'Viewers', value: 0 },
                    { label: 'Bitrate', value: '--' },
                    { label: 'FPS', value: '--' },
                  ]}
                />
              </Grid>
            </CardBody>
          </Card>
        </ContentArea>
      </MainContent>
    </AppShell>
  );
}

export default App;
