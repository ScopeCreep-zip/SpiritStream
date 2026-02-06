/**
 * Source Type Selector
 * Grid of source types for selecting which type of source to add
 */
import { useTranslation } from 'react-i18next';
import {
  Radio,
  Film,
  Monitor,
  AppWindow,
  Gamepad2,
  Camera,
  Usb,
  Mic,
  Palette,
  Type,
  Globe,
  ListVideo,
  Layers,
  Network,
  Zap,
  Shield,
} from 'lucide-react';
import type { SourceType } from '@/types/source';
import { getSourceTypeLabel } from '@/types/source';

export interface SourceTypeOption {
  type: SourceType;
  icon: React.ReactNode;
}

export const SOURCE_TYPE_OPTIONS: SourceTypeOption[] = [
  { type: 'rtmp', icon: <Radio className="w-5 h-5" /> },
  { type: 'mediaFile', icon: <Film className="w-5 h-5" /> },
  { type: 'screenCapture', icon: <Monitor className="w-5 h-5" /> },
  { type: 'windowCapture', icon: <AppWindow className="w-5 h-5" /> },
  { type: 'gameCapture', icon: <Gamepad2 className="w-5 h-5" /> },
  { type: 'camera', icon: <Camera className="w-5 h-5" /> },
  { type: 'captureCard', icon: <Usb className="w-5 h-5" /> },
  { type: 'audioDevice', icon: <Mic className="w-5 h-5" /> },
  { type: 'color', icon: <Palette className="w-5 h-5" /> },
  { type: 'text', icon: <Type className="w-5 h-5" /> },
  { type: 'browser', icon: <Globe className="w-5 h-5" /> },
  { type: 'mediaPlaylist', icon: <ListVideo className="w-5 h-5" /> },
  { type: 'nestedScene', icon: <Layers className="w-5 h-5" /> },
  { type: 'ndi', icon: <Network className="w-5 h-5" /> },
  { type: 'whip', icon: <Zap className="w-5 h-5" /> },
  { type: 'srt', icon: <Shield className="w-5 h-5" /> },
];

export function getSourceTypeDescription(type: SourceType): string {
  switch (type) {
    case 'rtmp':
      return 'Receive RTMP stream from encoder';
    case 'mediaFile':
      return 'Play local video or audio file';
    case 'screenCapture':
      return 'Capture entire display';
    case 'windowCapture':
      return 'Capture specific application window';
    case 'gameCapture':
      return 'Hardware-accelerated game capture';
    case 'camera':
      return 'Webcam or video device';
    case 'captureCard':
      return 'HDMI/SDI capture device';
    case 'audioDevice':
      return 'Microphone or line input';
    case 'color':
      return 'Solid color fill layer';
    case 'text':
      return 'Text overlay with styling';
    case 'browser':
      return 'Web page or widget';
    case 'mediaPlaylist':
      return 'Multiple media files in sequence';
    case 'nestedScene':
      return 'Embed another scene as a source';
    case 'ndi':
      return 'Receive NDI video over network';
    case 'whip':
      return 'Ultra-low latency WebRTC ingest (RFC 9725)';
    case 'srt':
      return 'Reliable UDP streaming for remote contribution';
  }
}

export interface SourceTypeSelectorProps {
  onSelect: (type: SourceType) => void;
  excludeTypes?: SourceType[];
}

export function SourceTypeSelector({ onSelect, excludeTypes = [] }: SourceTypeSelectorProps) {
  const { t } = useTranslation();

  const filteredTypes = SOURCE_TYPE_OPTIONS.filter(
    (item) => !excludeTypes.includes(item.type)
  );

  return (
    <div className="grid grid-cols-2 gap-3">
      {filteredTypes.map(({ type, icon }) => (
        <button
          key={type}
          className="flex items-start gap-3 p-3 rounded-lg border border-[var(--border)] hover:border-[var(--primary)] hover:bg-[var(--bg-hover)] transition-colors text-left"
          onClick={() => onSelect(type)}
        >
          <div className="flex-shrink-0 p-2 rounded-md bg-[var(--bg-elevated)] text-[var(--primary)]">
            {icon}
          </div>
          <div className="min-w-0">
            <div className="font-medium text-sm">{getSourceTypeLabel(type)}</div>
            <div className="text-xs text-muted truncate">
              {t(`stream.sourceDesc.${type}`, { defaultValue: getSourceTypeDescription(type) })}
            </div>
          </div>
        </button>
      ))}
    </div>
  );
}
