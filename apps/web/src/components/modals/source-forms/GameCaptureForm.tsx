import React from 'react';
import { useTranslation } from 'react-i18next';
import { Input } from '@/components/ui/Input';
import { Select, SelectOption } from '@/components/ui/Select';
import { Toggle } from '@/components/ui/Toggle';
import type { GameCaptureSource } from '@/types/source';

interface GameCaptureFormProps {
  data: GameCaptureSource;
  onChange: (data: GameCaptureSource) => void;
}

export const GameCaptureForm = React.memo(({ data, onChange }: GameCaptureFormProps) => {
  const { t } = useTranslation();

  const targetTypeOptions: SelectOption[] = [
    { value: 'any', label: t('stream.captureAnyGame', { defaultValue: 'Capture any fullscreen game' }) },
    { value: 'specific', label: t('stream.captureSpecificGame', { defaultValue: 'Capture specific game/window' }) },
  ];

  const captureModeOptions: SelectOption[] = [
    { value: 'auto', label: t('stream.captureModeAuto', { defaultValue: 'Auto (Recommended)' }) },
    { value: 'dxgi', label: 'DXGI (Windows)' },
    { value: 'opengl', label: 'OpenGL' },
    { value: 'bitblt', label: 'BitBlt (Legacy)' },
  ];

  return (
    <div className="flex flex-col gap-4">
      <Input
        label={t('stream.sourceName', { defaultValue: 'Source Name' })}
        value={data.name}
        onChange={(e) => onChange({ ...data, name: e.target.value })}
        placeholder="Game Capture"
      />
      <Select
        label={t('stream.targetType', { defaultValue: 'Target' })}
        value={data.targetType}
        onChange={(e) => onChange({ ...data, targetType: e.target.value as 'any' | 'specific' })}
        options={targetTypeOptions}
      />
      {data.targetType === 'specific' && (
        <>
          <Input
            label={t('stream.windowTitle', { defaultValue: 'Window Title (partial match)' })}
            value={data.windowTitle || ''}
            onChange={(e) => onChange({ ...data, windowTitle: e.target.value })}
            placeholder="Minecraft"
          />
          <Input
            label={t('stream.processName', { defaultValue: 'Process Name (optional)' })}
            value={data.processName || ''}
            onChange={(e) => onChange({ ...data, processName: e.target.value })}
            placeholder="javaw.exe"
          />
        </>
      )}
      <Select
        label={t('stream.captureMode', { defaultValue: 'Capture Mode' })}
        value={data.captureMode}
        onChange={(e) => onChange({ ...data, captureMode: e.target.value as 'auto' | 'bitblt' | 'dxgi' | 'opengl' })}
        options={captureModeOptions}
      />
      <Input
        label={t('stream.fps', { defaultValue: 'Frame Rate' })}
        type="number"
        value={String(data.fps)}
        onChange={(e) => onChange({ ...data, fps: parseInt(e.target.value) || 60 })}
      />
      <div className="flex items-center justify-between">
        <span className="text-sm">{t('stream.captureCursor', { defaultValue: 'Capture Cursor' })}</span>
        <Toggle
          checked={data.captureCursor}
          onChange={(checked) => onChange({ ...data, captureCursor: checked })}
        />
      </div>
      <div className="flex items-center justify-between">
        <div>
          <span className="text-sm">{t('stream.antiCheatHook', { defaultValue: 'Anti-Cheat Compatible' })}</span>
          <p className="text-xs text-muted">
            {t('stream.antiCheatHelper', { defaultValue: 'May reduce performance but works with anti-cheat software' })}
          </p>
        </div>
        <Toggle
          checked={data.antiCheatHook}
          onChange={(checked) => onChange({ ...data, antiCheatHook: checked })}
        />
      </div>
      <div className="flex items-center justify-between">
        <span className="text-sm">{t('stream.captureAudio', { defaultValue: 'Capture Audio' })}</span>
        <Toggle
          checked={data.captureAudio}
          onChange={(checked) => onChange({ ...data, captureAudio: checked })}
        />
      </div>
    </div>
  );
});

GameCaptureForm.displayName = 'GameCaptureForm';
