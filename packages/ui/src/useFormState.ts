import { useCallback, useState } from 'react';

/**
 * `useFormState(initialData)` — minimal controlled-form state hook.
 * Replaces the ad-hoc `{...form, [key]: value}` spread
 * pattern that ProfileModal / TargetModal / OutputGroupModal repeated
 * four times.
 *
 * Returns:
 * - `values` — current snapshot.
 * - `set(key, value)` — single-field update.
 * - `merge(partial)` — multi-field update.
 * - `reset(next?)` — restore to `initialData` (or supplied state).
 * - `dirty` — `true` iff `values !== initialData` (referentially).
 */
export interface FormStateHandle<T> {
  values: T;
  set: <K extends keyof T>(key: K, value: T[K]) => void;
  merge: (partial: Partial<T>) => void;
  reset: (next?: T) => void;
  dirty: boolean;
}

export function useFormState<T extends object>(initialData: T): FormStateHandle<T> {
  const [values, setValues] = useState<T>(initialData);
  const [initial, setInitial] = useState<T>(initialData);

  const set = useCallback(<K extends keyof T>(key: K, value: T[K]) => {
    setValues((prev) => ({ ...prev, [key]: value }));
  }, []);

  const merge = useCallback((partial: Partial<T>) => {
    setValues((prev) => ({ ...prev, ...partial }));
  }, []);

  const reset = useCallback(
    (next?: T) => {
      const target = next ?? initial;
      setValues(target);
      if (next) setInitial(next);
    },
    [initial],
  );

  return {
    values,
    set,
    merge,
    reset,
    dirty: values !== initial,
  };
}
