import {
  Radio,
  Film,
  Monitor,
  Camera,
  Usb,
  Mic,
  Palette,
  Type,
  Globe,
} from 'lucide-react';
import type { Source } from '@/types/profile';

export const SourceIcon = ({ type }: { type: Source['type'] }) => {
  const iconClass = 'w-4 h-4';
  switch (type) {
    case 'rtmp':
      return <Radio className={iconClass} />;
    case 'mediaFile':
      return <Film className={iconClass} />;
    case 'screenCapture':
      return <Monitor className={iconClass} />;
    case 'camera':
      return <Camera className={iconClass} />;
    case 'captureCard':
      return <Usb className={iconClass} />;
    case 'audioDevice':
      return <Mic className={iconClass} />;
    case 'color':
      return <Palette className={iconClass} />;
    case 'text':
      return <Type className={iconClass} />;
    case 'browser':
      return <Globe className={iconClass} />;
    default:
      return null;
  }
};
