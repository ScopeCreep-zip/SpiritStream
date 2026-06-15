import { cn } from '@/lib/cn';
import type { Platform } from '@spiritstream/types';
import { PLATFORMS } from '@/lib/profile-helpers';
import { ServiceMark } from '@/components/stream/ServiceMark';
import { brandSlug } from '@/lib/serviceLogos';

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

  return (
    <div
      className={cn(
        'rounded-md flex items-center justify-center font-semibold',
        'bg-[var(--platform-bg)] text-[var(--platform-fg)]',
        sizeStyles[size],
        className
      )}
      // Per-platform brand colors come from PLATFORMS config data, so they
      // can't be static tokens — inject them as CSS vars (Modal.tsx pattern).
      style={
        {
          '--platform-bg': platformConfig.color,
          '--platform-fg': platformConfig.textColor,
        } as React.CSSProperties
      }
    >
      <ServiceMark
        slug={brandSlug(platformConfig.displayName)}
        abbreviation={platformConfig.abbreviation}
      />
    </div>
  );
}
