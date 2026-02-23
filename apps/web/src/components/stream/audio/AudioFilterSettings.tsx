/**
 * AudioFilterSettings — Per-filter parameter editor panel
 * Extracted from AudioFilterButton inline sub-component
 */
import { useTranslation } from 'react-i18next';
import type { AudioFilter, Source } from '@/types/source';
import { getAudioFilterLabel } from '@/types/source';
import { ParamSlider } from './ParamSlider';

interface AudioFilterSettingsProps {
  filter: AudioFilter;
  onUpdate: (updates: Partial<AudioFilter>) => void;
  sidechainOptions?: Source[];
}

export function AudioFilterSettings({ filter, onUpdate, sidechainOptions = [] }: AudioFilterSettingsProps) {
  const { t } = useTranslation();

  return (
    <div className="p-3 border-t border-[var(--border-default)] bg-[var(--bg-sunken)] space-y-3">
      <h5 className="text-[10px] font-medium text-[var(--text-muted)] uppercase">
        {getAudioFilterLabel(filter.type)} {t('common.settings', { defaultValue: 'Settings' })}
      </h5>

      {filter.type === 'gain' && (
        <ParamSlider label={t('audio.gain', { defaultValue: 'Gain' })} value={filter.gain} min={-30} max={30} step={0.5} unit=" dB" onChange={(v) => onUpdate({ gain: v })} />
      )}

      {filter.type === 'compressor' && (
        <>
          <ParamSlider label={t('audio.threshold', { defaultValue: 'Threshold' })} value={filter.threshold} min={-60} max={0} step={1} unit=" dB" onChange={(v) => onUpdate({ threshold: v })} />
          <ParamSlider label={t('audio.ratio', { defaultValue: 'Ratio' })} value={filter.ratio} min={1} max={32} step={0.5} unit=":1" onChange={(v) => onUpdate({ ratio: v })} />
          <ParamSlider label={t('audio.attack', { defaultValue: 'Attack' })} value={filter.attack} min={0} max={500} step={1} unit=" ms" onChange={(v) => onUpdate({ attack: v })} />
          <ParamSlider label={t('audio.release', { defaultValue: 'Release' })} value={filter.release} min={0} max={1000} step={1} unit=" ms" onChange={(v) => onUpdate({ release: v })} />
          <ParamSlider label={t('audio.outputGain', { defaultValue: 'Output Gain' })} value={filter.outputGain} min={-30} max={30} step={0.5} unit=" dB" onChange={(v) => onUpdate({ outputGain: v })} />

          {/* Sidechain Source Selector for Audio Ducking */}
          <div className="space-y-1 pt-2 border-t border-[var(--border-muted)]">
            <div className="flex justify-between text-[10px]">
              <span className="text-[var(--text-muted)]">
                {t('audio.sidechainSource', { defaultValue: 'Sidechain Source' })}
              </span>
            </div>
            <select
              value={filter.sidechainSourceId || ''}
              onChange={(e) => onUpdate({ sidechainSourceId: e.target.value || undefined })}
              className="w-full h-6 px-1.5 text-[10px] bg-[var(--bg-base)] border border-[var(--border-default)] rounded text-[var(--text-secondary)] focus:outline-none focus:ring-1 focus:ring-primary/50"
            >
              <option value="">{t('audio.noSidechain', { defaultValue: 'None (no ducking)' })}</option>
              {sidechainOptions.map((source) => (
                <option key={source.id} value={source.id}>{source.name}</option>
              ))}
            </select>
            <p className="text-[9px] text-[var(--text-muted)] leading-tight">
              {t('audio.sidechainHelp', { defaultValue: 'When sidechain source is active, this track will be ducked (audio ducking).' })}
            </p>
          </div>
        </>
      )}

      {filter.type === 'noiseGate' && (
        <>
          <ParamSlider label={t('audio.threshold', { defaultValue: 'Threshold' })} value={filter.threshold} min={-60} max={0} step={1} unit=" dB" onChange={(v) => onUpdate({ threshold: v })} />
          <ParamSlider label={t('audio.attack', { defaultValue: 'Attack' })} value={filter.attack} min={0} max={100} step={1} unit=" ms" onChange={(v) => onUpdate({ attack: v })} />
          <ParamSlider label={t('audio.hold', { defaultValue: 'Hold' })} value={filter.hold} min={0} max={500} step={1} unit=" ms" onChange={(v) => onUpdate({ hold: v })} />
          <ParamSlider label={t('audio.release', { defaultValue: 'Release' })} value={filter.release} min={0} max={1000} step={1} unit=" ms" onChange={(v) => onUpdate({ release: v })} />
        </>
      )}

      {filter.type === 'noiseSuppression' && (
        <ParamSlider label={t('audio.suppressionLevel', { defaultValue: 'Suppression Level' })} value={filter.level} min={0} max={100} step={1} unit="%" onChange={(v) => onUpdate({ level: v })} />
      )}

      {filter.type === 'expander' && (
        <>
          <ParamSlider label={t('audio.threshold', { defaultValue: 'Threshold' })} value={filter.threshold} min={-60} max={0} step={1} unit=" dB" onChange={(v) => onUpdate({ threshold: v })} />
          <ParamSlider label={t('audio.ratio', { defaultValue: 'Ratio' })} value={filter.ratio} min={1} max={10} step={0.5} unit=":1" onChange={(v) => onUpdate({ ratio: v })} />
          <ParamSlider label={t('audio.attack', { defaultValue: 'Attack' })} value={filter.attack} min={0} max={100} step={1} unit=" ms" onChange={(v) => onUpdate({ attack: v })} />
          <ParamSlider label={t('audio.release', { defaultValue: 'Release' })} value={filter.release} min={0} max={500} step={1} unit=" ms" onChange={(v) => onUpdate({ release: v })} />
        </>
      )}
    </div>
  );
}
