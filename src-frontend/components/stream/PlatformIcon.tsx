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

  // Use the color from PLATFORMS constant
  const bgStyle = { backgroundColor: platformConfig.color };

  return (
    <div
      className={cn(
        'rounded-md flex items-center justify-center font-semibold',
        sizeStyles[size],
        className
      )}
      style={{
        ...bgStyle,
        color: platformConfig.textColor,
      }}
    >
      {platformConfig.abbreviation}
    </div>
  );
}
