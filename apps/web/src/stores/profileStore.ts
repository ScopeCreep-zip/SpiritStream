/// Re-export shim — the profile store now lives in `./profile/`. Existing
/// imports from `@/stores/profileStore` continue to resolve through this
/// file; new code should import from `@/stores/profile` directly.
export {
  useProfileStore,
  subscribeProfileActivated,
  subscribeOAuthTokenExpired,
} from './profile';
export type { ProfileState } from './profile';
