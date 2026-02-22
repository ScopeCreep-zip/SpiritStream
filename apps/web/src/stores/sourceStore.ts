/**
 * Source Store (backward compatibility re-export)
 *
 * Device discovery → useDeviceStore (deviceStore.ts)
 * Source CRUD → useProfileStore (profileStore.ts)
 *
 * This file re-exports useDeviceStore as useSourceStore so existing consumers
 * continue to work during migration. New code should import from the
 * canonical stores directly.
 */
export { useDeviceStore as useSourceStore } from './deviceStore';
export type { DeviceDiscoveryState } from './deviceStore';
