import React, { useCallback } from 'react';
import * as Menubar from '@radix-ui/react-menubar';
import { useTranslation } from 'react-i18next';
import { api } from '@/lib/client';
import { toast } from '@/hooks/useToast';
import { logger } from '@/lib/logger';
import { useProfileStore } from '@/stores/profileStore';
import { useStreamStore } from '@/stores/streamStore';
import { incomingRtmpUrl } from '@/lib/profile-helpers';
import { testStreamConnectivity } from '@/lib/streamConnectivity';
import {
  TRIGGER_CLASS,
  CONTENT_CLASS,
  ITEM_CLASS,
  SHORTCUT_CLASS,
  SEPARATOR_CLASS,
} from './menuStyles';

interface StreamMenuProps {
  /** Stream → Encoder Settings dispatches here so the shell can target the active group. */
  onEditEncoder: () => void;
  /** Whether an encoder is available to edit (i.e. an output group exists on the active profile). */
  canEditEncoder: boolean;
}

export function StreamMenu({ onEditEncoder, canEditEncoder }: StreamMenuProps): React.ReactElement {
  const { t } = useTranslation();
  const current = useProfileStore((s) => s.current);
  const isStreaming = useStreamStore((s) => s.isStreaming);
  const startAllGroups = useStreamStore((s) => s.startAllGroups);
  const stopAllGroups = useStreamStore((s) => s.stopAllGroups);

  const handleStart = useCallback(async (): Promise<void> => {
    if (!current) return;
    try {
      await api.stream.validate(current);
      await startAllGroups(current.outputGroups, incomingRtmpUrl(current.input));
      toast.success(t('toast.streamStarted'));
    } catch (err) {
      logger.error('[menu] start failed', err);
      toast.error(
        t('toast.startFailed', { error: err instanceof Error ? err.message : String(err) }),
      );
    }
  }, [current, startAllGroups, t]);

  const handleTestConnectivity = useCallback(async (): Promise<void> => {
    if (!current) {
      toast.error(t('errors.noProfileSelected'));
      return;
    }
    try {
      toast.info(t('toast.testingConnectivity', { count: 0 }));
      const result = await testStreamConnectivity(current, { enabledTargetsOnly: false });
      if (result.allPassed) {
        toast.success(t('toast.allTestsPassed', { count: result.passed }));
      } else {
        toast.error(t('toast.someTestsFailed', { passed: result.passed, failed: result.failed }));
      }
    } catch (err) {
      toast.error(
        t('toast.testFailed', { error: err instanceof Error ? err.message : String(err) }),
      );
    }
  }, [current, t]);

  return (
    <Menubar.Menu>
      <Menubar.Trigger className={TRIGGER_CLASS}>
        {t('menu.stream', { defaultValue: 'Stream' })}
      </Menubar.Trigger>
      <Menubar.Portal>
        <Menubar.Content className={CONTENT_CLASS} align="start" sideOffset={4}>
          <Menubar.Item
            className={ITEM_CLASS}
            disabled={!current || isStreaming}
            onSelect={handleStart}
          >
            <span>{t('menu.stream.start', { defaultValue: 'Start streaming' })}</span>
            <span className={SHORTCUT_CLASS}>⌘↵</span>
          </Menubar.Item>
          <Menubar.Item className={ITEM_CLASS} disabled={!isStreaming} onSelect={() => stopAllGroups()}>
            <span>{t('menu.stream.stop', { defaultValue: 'Stop streaming' })}</span>
            <span className={SHORTCUT_CLASS}>⌘.</span>
          </Menubar.Item>
          <Menubar.Separator className={SEPARATOR_CLASS} />
          <Menubar.Item
            className={ITEM_CLASS}
            disabled={!current || isStreaming}
            onSelect={handleTestConnectivity}
          >
            {t('menu.stream.test', { defaultValue: 'Test connectivity' })}
          </Menubar.Item>
          <Menubar.Separator className={SEPARATOR_CLASS} />
          <Menubar.Item className={ITEM_CLASS} disabled={!canEditEncoder} onSelect={onEditEncoder}>
            {t('menu.stream.encoder', { defaultValue: 'Encoder settings…' })}
          </Menubar.Item>
        </Menubar.Content>
      </Menubar.Portal>
    </Menubar.Menu>
  );
}
