/**
 * Vitest global setup. Loaded once before any test file via the
 * `setupFiles` entry in `vite.config.ts`.
 *
 * Adds:
 * - `@testing-library/jest-dom` matchers (`toBeInTheDocument`,
 *   `toHaveTextContent`, etc) on the Vitest `expect`.
 * - An `afterEach(cleanup)` so each component test starts with an empty
 *   DOM. This config doesn't set `globals: true`, so RTL's automatic
 *   cleanup never registers — without this, a render from one test
 *   leaks into the next and `getByText` finds duplicates.
 * - Stub `window.matchMedia` so components that probe the user's
 *   reduced-motion / prefers-color-scheme preferences don't blow up
 *   under jsdom (which doesn't implement matchMedia).
 * - Install an in-memory `localStorage` so Zustand `persist` stores
 *   (chatStore, themeStore, …) can rehydrate/write. This jsdom build
 *   exposes a `localStorage` object whose `setItem` is `undefined`,
 *   which makes `persist` throw on the first `setState`.
 */
import '@testing-library/jest-dom/vitest';
import { afterEach } from 'vitest';
import { cleanup } from '@testing-library/react';

afterEach(() => {
  cleanup();
});

if (typeof window !== 'undefined' && typeof window.localStorage?.setItem !== 'function') {
  const store = new Map<string, string>();
  const memoryStorage: Storage = {
    get length() {
      return store.size;
    },
    clear: () => store.clear(),
    getItem: (key) => (store.has(key) ? store.get(key)! : null),
    key: (index) => Array.from(store.keys())[index] ?? null,
    removeItem: (key) => {
      store.delete(key);
    },
    setItem: (key, value) => {
      store.set(key, String(value));
    },
  };
  Object.defineProperty(window, 'localStorage', {
    configurable: true,
    writable: true,
    value: memoryStorage,
  });
}

if (typeof window !== 'undefined' && !window.matchMedia) {
  Object.defineProperty(window, 'matchMedia', {
    writable: true,
    value: (query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: () => {},
      removeListener: () => {},
      addEventListener: () => {},
      removeEventListener: () => {},
      dispatchEvent: () => false,
    }),
  });
}
