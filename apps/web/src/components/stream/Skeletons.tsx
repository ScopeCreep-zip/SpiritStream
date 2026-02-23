/**
 * Loading skeleton fallbacks for lazy-loaded stream components
 */

export const PanelSkeleton = () => (
  <div className="h-full bg-[var(--bg-surface)] rounded-lg animate-pulse">
    <div className="h-10 bg-[var(--bg-elevated)] rounded-t-lg" />
    <div className="p-4 space-y-3">
      <div className="h-4 bg-[var(--bg-elevated)] rounded w-3/4" />
      <div className="h-4 bg-[var(--bg-elevated)] rounded w-1/2" />
      <div className="h-4 bg-[var(--bg-elevated)] rounded w-2/3" />
    </div>
  </div>
);

export const StudioLayoutSkeleton = () => (
  <div className="flex-1 bg-[var(--bg-surface)] rounded-lg animate-pulse flex items-center justify-center">
    <div className="text-[var(--text-muted)]">Loading Studio Mode...</div>
  </div>
);
