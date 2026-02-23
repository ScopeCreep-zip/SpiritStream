/**
 * Filter parameter editors — per-filter-type parameter components
 * Extracted from AudioFilterChainEditor
 *
 * Note: This uses an inline-layout ParamSlider (horizontal: label | slider | value)
 * distinct from the popover-style ParamSlider in ./ParamSlider.tsx (vertical layout)
 */
import type {
  AudioFilter,
  CompressorFilter,
  NoiseGateFilter,
  NoiseSuppressionFilter,
  GainFilter,
  ExpanderFilter,
} from '@/types/source';

/** Inline-layout slider for filter chain editor (label | slider | value in a row) */
function InlineParamSlider({ label, value, min, max, step, unit, onChange }: {
  label: string; value: number; min: number; max: number; step: number; unit: string;
  onChange: (val: number) => void;
}) {
  return (
    <div className="flex items-center gap-2 py-0.5">
      <span className="text-[10px] text-[var(--text-muted)] w-20 shrink-0">{label}</span>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(parseFloat(e.target.value))}
        className="flex-1 h-1 accent-[var(--primary)]"
      />
      <span className="text-[10px] text-[var(--text-secondary)] tabular-nums w-14 text-right">
        {value.toFixed(step < 1 ? 1 : 0)}{unit}
      </span>
    </div>
  );
}

export function FilterParams({ filter, onUpdate }: { filter: AudioFilter; onUpdate: (updates: Partial<AudioFilter>) => void }) {
  switch (filter.type) {
    case 'gain':
      return <GainParams filter={filter} onUpdate={onUpdate} />;
    case 'compressor':
      return <CompressorParams filter={filter} onUpdate={onUpdate} />;
    case 'noiseGate':
      return <NoiseGateParams filter={filter} onUpdate={onUpdate} />;
    case 'noiseSuppression':
      return <NoiseSuppressionParams filter={filter} onUpdate={onUpdate} />;
    case 'expander':
      return <ExpanderParams filter={filter} onUpdate={onUpdate} />;
    default:
      return null;
  }
}

function GainParams({ filter, onUpdate }: { filter: GainFilter; onUpdate: (u: Partial<GainFilter>) => void }) {
  return <InlineParamSlider label="Gain" value={filter.gain} min={-30} max={30} step={0.1} unit=" dB" onChange={(gain) => onUpdate({ gain })} />;
}

function CompressorParams({ filter, onUpdate }: { filter: CompressorFilter; onUpdate: (u: Partial<CompressorFilter>) => void }) {
  return (
    <div className="flex flex-col">
      <InlineParamSlider label="Threshold" value={filter.threshold} min={-60} max={0} step={0.5} unit=" dB" onChange={(threshold) => onUpdate({ threshold })} />
      <InlineParamSlider label="Ratio" value={filter.ratio} min={1} max={32} step={0.5} unit=":1" onChange={(ratio) => onUpdate({ ratio })} />
      <InlineParamSlider label="Attack" value={filter.attack} min={0} max={500} step={1} unit=" ms" onChange={(attack) => onUpdate({ attack })} />
      <InlineParamSlider label="Release" value={filter.release} min={0} max={1000} step={1} unit=" ms" onChange={(release) => onUpdate({ release })} />
      <InlineParamSlider label="Output Gain" value={filter.outputGain} min={-30} max={30} step={0.1} unit=" dB" onChange={(outputGain) => onUpdate({ outputGain })} />
    </div>
  );
}

function NoiseGateParams({ filter, onUpdate }: { filter: NoiseGateFilter; onUpdate: (u: Partial<NoiseGateFilter>) => void }) {
  return (
    <div className="flex flex-col">
      <InlineParamSlider label="Threshold" value={filter.threshold} min={-60} max={0} step={0.5} unit=" dB" onChange={(threshold) => onUpdate({ threshold })} />
      <InlineParamSlider label="Attack" value={filter.attack} min={0} max={100} step={1} unit=" ms" onChange={(attack) => onUpdate({ attack })} />
      <InlineParamSlider label="Hold" value={filter.hold} min={0} max={500} step={1} unit=" ms" onChange={(hold) => onUpdate({ hold })} />
      <InlineParamSlider label="Release" value={filter.release} min={0} max={1000} step={1} unit=" ms" onChange={(release) => onUpdate({ release })} />
    </div>
  );
}

function NoiseSuppressionParams({ filter, onUpdate }: { filter: NoiseSuppressionFilter; onUpdate: (u: Partial<NoiseSuppressionFilter>) => void }) {
  return <InlineParamSlider label="Level" value={filter.level} min={0} max={100} step={1} unit="%" onChange={(level) => onUpdate({ level })} />;
}

function ExpanderParams({ filter, onUpdate }: { filter: ExpanderFilter; onUpdate: (u: Partial<ExpanderFilter>) => void }) {
  return (
    <div className="flex flex-col">
      <InlineParamSlider label="Threshold" value={filter.threshold} min={-60} max={0} step={0.5} unit=" dB" onChange={(threshold) => onUpdate({ threshold })} />
      <InlineParamSlider label="Ratio" value={filter.ratio} min={1} max={10} step={0.1} unit=":1" onChange={(ratio) => onUpdate({ ratio })} />
      <InlineParamSlider label="Attack" value={filter.attack} min={0} max={100} step={1} unit=" ms" onChange={(attack) => onUpdate({ attack })} />
      <InlineParamSlider label="Release" value={filter.release} min={0} max={500} step={1} unit=" ms" onChange={(release) => onUpdate({ release })} />
    </div>
  );
}
