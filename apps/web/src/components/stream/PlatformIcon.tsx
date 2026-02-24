import { useCallback } from 'react';
import { cn } from '@/lib/cn';
import { type Platform, PLATFORMS } from '@/types/profile';

export interface PlatformIconProps {
  platform: Platform;
  size?: 'sm' | 'md' | 'lg';
  className?: string;
}

const sizeStyles = {
  sm: 'w-6 h-6 text-[0.625rem]',
  md: 'w-8 h-8 text-xs',
  lg: 'w-10 h-10 text-sm',
};

export function PlatformIcon({ platform, size = 'md', className }: PlatformIconProps) {
  const platformConfig = PLATFORMS[platform];

  const setColors = useCallback(
    (el: HTMLDivElement | null) => {
      if (el) {
        el.style.setProperty('background-color', platformConfig.color);
        el.style.setProperty('color', platformConfig.textColor);
      }
    },
    [platformConfig.color, platformConfig.textColor]
  );

  return (
    <div
      ref={setColors}
      className={cn(
        'rounded-md flex items-center justify-center font-semibold',
        sizeStyles[size],
        className
      )}
    >
      {platformConfig.abbreviation}
    </div>
  );
}
