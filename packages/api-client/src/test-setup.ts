// This jsdom build exposes `window.localStorage` as an object whose
// methods are undefined, so `setItem`/`removeItem` throw. The api-client
// config helpers call them directly (no try/catch), so install a working
// in-memory polyfill when the real one is non-functional. Guarded so a
// working localStorage is never clobbered.
if (typeof window !== 'undefined' && typeof window.localStorage?.setItem !== 'function') {
  const store = new Map<string, string>();
  const memoryStorage: Storage = {
    get length() {
      return store.size;
    },
    clear: () => store.clear(),
    getItem: (key) => (store.has(key) ? store.get(key)! : null),
    key: (index) => Array.from(store.keys())[index] ?? null,
    removeItem: (key) => void store.delete(key),
    setItem: (key, value) => void store.set(key, String(value)),
  };
  Object.defineProperty(window, 'localStorage', {
    configurable: true,
    writable: true,
    value: memoryStorage,
  });
}
