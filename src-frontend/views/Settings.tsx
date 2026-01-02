import { useState } from 'react';
import { FolderOpen, Download, Trash2, Github, BookOpen, RefreshCw } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardDescription, CardBody } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Select } from '@/components/ui/Select';
import { Input } from '@/components/ui/Input';
import { Toggle } from '@/components/ui/Toggle';
import { Grid } from '@/components/ui/Grid';
import { Logo } from '@/components/layout/Logo';

interface SettingsState {
  // General
  language: string;
  startMinimized: boolean;
  showNotifications: boolean;
  // FFmpeg
  ffmpegPath: string;
  ffmpegVersion: string;
  autoDownloadFfmpeg: boolean;
  // Data & Privacy
  profileStoragePath: string;
  encryptStreamKeys: boolean;
}

const defaultSettings: SettingsState = {
  language: 'en',
  startMinimized: false,
  showNotifications: true,
  ffmpegPath: '/usr/local/bin/ffmpeg',
  ffmpegVersion: '6.1.0',
  autoDownloadFfmpeg: true,
  profileStoragePath: '~/Library/Application Support/MagillaStream/profiles',
  encryptStreamKeys: true,
};

export function Settings() {
  const [settings, setSettings] = useState<SettingsState>(defaultSettings);

  const updateSetting = <K extends keyof SettingsState>(key: K, value: SettingsState[K]) => {
    setSettings(prev => ({ ...prev, [key]: value }));
    // TODO: Persist to Tauri backend
  };

  const handleBrowseFfmpeg = () => {
    // TODO: Open Tauri file dialog
    console.log('Browse for FFmpeg');
  };

  const handleOpenProfileStorage = () => {
    // TODO: Open folder in system file manager
    console.log('Open profile storage');
  };

  const handleExportData = () => {
    // TODO: Export all data via Tauri
    console.log('Export data');
  };

  const handleClearAllData = () => {
    // TODO: Show confirmation dialog, then clear data
    if (confirm('Are you sure you want to clear all data? This cannot be undone.')) {
      console.log('Clear all data');
    }
  };

  const languageOptions = [
    { value: 'en', label: 'English' },
    { value: 'es', label: 'Español' },
    { value: 'fr', label: 'Français' },
    { value: 'de', label: 'Deutsch' },
    { value: 'ja', label: '日本語' },
  ];

  return (
    <Grid cols={2}>
      {/* General Settings */}
      <Card>
        <CardHeader>
          <div>
            <CardTitle>General Settings</CardTitle>
            <CardDescription>
              Configure application preferences
            </CardDescription>
          </div>
        </CardHeader>
        <CardBody style={{ padding: '24px', display: 'flex', flexDirection: 'column', gap: '16px' }}>
          <Select
            label="Language"
            value={settings.language}
            onChange={(e) => updateSetting('language', e.target.value)}
            options={languageOptions}
          />
          <div className="flex items-center justify-between" style={{ padding: '8px 0' }}>
            <div>
              <div className="text-sm font-medium text-[var(--text-primary)]">
                Start Minimized
              </div>
              <div className="text-xs text-[var(--text-tertiary)]">
                Start the app minimized to system tray
              </div>
            </div>
            <Toggle
              checked={settings.startMinimized}
              onChange={(checked) => updateSetting('startMinimized', checked)}
            />
          </div>
          <div className="flex items-center justify-between" style={{ padding: '8px 0' }}>
            <div>
              <div className="text-sm font-medium text-[var(--text-primary)]">
                Show Notifications
              </div>
              <div className="text-xs text-[var(--text-tertiary)]">
                Display system notifications for stream events
              </div>
            </div>
            <Toggle
              checked={settings.showNotifications}
              onChange={(checked) => updateSetting('showNotifications', checked)}
            />
          </div>
        </CardBody>
      </Card>

      {/* FFmpeg Configuration */}
      <Card>
        <CardHeader>
          <div>
            <CardTitle>FFmpeg Configuration</CardTitle>
            <CardDescription>
              Configure FFmpeg encoding backend
            </CardDescription>
          </div>
        </CardHeader>
        <CardBody style={{ padding: '24px', display: 'flex', flexDirection: 'column', gap: '16px' }}>
          <div className="flex flex-col" style={{ gap: '6px' }}>
            <label className="block text-sm font-medium text-[var(--text-primary)]">
              FFmpeg Path
            </label>
            <div className="flex" style={{ gap: '8px' }}>
              <Input
                value={settings.ffmpegPath}
                onChange={(e) => updateSetting('ffmpegPath', e.target.value)}
                className="flex-1"
              />
              <Button variant="outline" onClick={handleBrowseFfmpeg}>
                <FolderOpen className="w-4 h-4" />
                Browse
              </Button>
            </div>
          </div>
          <Input
            label="FFmpeg Version"
            value={settings.ffmpegVersion}
            disabled
            helper="Detected version of FFmpeg"
          />
          <div className="flex items-center justify-between" style={{ padding: '8px 0' }}>
            <div>
              <div className="text-sm font-medium text-[var(--text-primary)]">
                Auto-Download FFmpeg
              </div>
              <div className="text-xs text-[var(--text-tertiary)]">
                Automatically download FFmpeg if not found
              </div>
            </div>
            <Toggle
              checked={settings.autoDownloadFfmpeg}
              onChange={(checked) => updateSetting('autoDownloadFfmpeg', checked)}
            />
          </div>
        </CardBody>
      </Card>

      {/* Data & Privacy */}
      <Card>
        <CardHeader>
          <div>
            <CardTitle>Data & Privacy</CardTitle>
            <CardDescription>
              Manage your data and privacy settings
            </CardDescription>
          </div>
        </CardHeader>
        <CardBody style={{ padding: '24px', display: 'flex', flexDirection: 'column', gap: '16px' }}>
          <div className="flex flex-col" style={{ gap: '6px' }}>
            <label className="block text-sm font-medium text-[var(--text-primary)]">
              Profile Storage
            </label>
            <div className="flex" style={{ gap: '8px' }}>
              <Input
                value={settings.profileStoragePath}
                disabled
                className="flex-1 font-mono text-xs"
              />
              <Button variant="outline" onClick={handleOpenProfileStorage}>
                <FolderOpen className="w-4 h-4" />
                Open
              </Button>
            </div>
          </div>
          <div className="flex items-center justify-between" style={{ padding: '8px 0' }}>
            <div>
              <div className="text-sm font-medium text-[var(--text-primary)]">
                Encrypt Stream Keys
              </div>
              <div className="text-xs text-[var(--text-tertiary)]">
                Encrypt stream keys at rest using AES-256
              </div>
            </div>
            <Toggle
              checked={settings.encryptStreamKeys}
              onChange={(checked) => updateSetting('encryptStreamKeys', checked)}
            />
          </div>
          <div className="border-t border-[var(--border-muted)] flex" style={{ paddingTop: '16px', gap: '12px' }}>
            <Button variant="outline" onClick={handleExportData}>
              <Download className="w-4 h-4" />
              Export Data
            </Button>
            <Button variant="destructive" onClick={handleClearAllData}>
              <Trash2 className="w-4 h-4" />
              Clear All Data
            </Button>
          </div>
        </CardBody>
      </Card>

      {/* About */}
      <Card>
        <CardHeader>
          <div>
            <CardTitle>About</CardTitle>
            <CardDescription>
              Application information
            </CardDescription>
          </div>
        </CardHeader>
        <CardBody>
          <div className="text-center" style={{ padding: '16px 0' }}>
            <div className="flex justify-center" style={{ marginBottom: '16px' }}>
              <Logo size="lg" />
            </div>
            <div className="text-sm text-[var(--text-secondary)]" style={{ marginBottom: '4px' }}>
              Version 2.0.0-beta
            </div>
            <div className="text-xs text-[var(--text-tertiary)]" style={{ marginBottom: '24px' }}>
              Multi-destination streaming made simple
            </div>
            <div className="flex justify-center" style={{ gap: '12px' }}>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => window.open('https://github.com/billboyles/magillastream', '_blank')}
              >
                <Github className="w-4 h-4" />
                GitHub
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => console.log('Open docs')}
              >
                <BookOpen className="w-4 h-4" />
                Docs
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => console.log('Check updates')}
              >
                <RefreshCw className="w-4 h-4" />
                Updates
              </Button>
            </div>
          </div>
        </CardBody>
      </Card>
    </Grid>
  );
}
