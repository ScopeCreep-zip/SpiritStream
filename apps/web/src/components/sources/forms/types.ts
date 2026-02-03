/**
 * Shared types for source form components
 */
import type { Source } from '@/types/source';

/**
 * Common props for all source form components
 */
export interface SourceFormProps<T extends Source> {
  /** Current form data */
  data: T;
  /** Called when form data changes */
  onChange: (data: T) => void;
  /** Whether devices are being discovered */
  isDiscovering?: boolean;
  /** Callback to refresh devices */
  onRefreshDevices?: () => void;
}
