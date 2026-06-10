import { useCallback, useMemo, useState } from 'react';

/**
 * `useFormValidation(rules)` — helper that consolidates the
 * "validate field-by-field, surface errors per field" pattern that
 * ProfileModal / TargetModal / OutputGroupModal each implemented
 * with subtle variations.
 *
 * Rules are functions that take the current values and return either
 * `null` (clear) or an error message string. The hook collects them
 * into a `{ field: errorMessage }` map and exposes a `validate()`
 * function that runs every rule and returns whether the form is OK.
 *
 * Deliberately minimal — no async validation, no schema-library
 * coupling. Larger forms can layer Zod / Valibot on top by writing
 * rules that delegate to the schema's `safeParse`.
 */
export type ValidationRule<T> = (values: T) => string | null;

export interface FormValidationHandle<T> {
  /** Current per-field error map (empty when `validate` hasn't been called). */
  errors: Partial<Record<keyof T, string>>;
  /** True iff every rule passes. Does NOT mutate `errors`. */
  isValid: boolean;
  /**
   * Run every rule and store the results in `errors`. Returns true
   * when every rule passed. Use this on submit.
   */
  validate: () => boolean;
  /** Clear all stored errors (e.g. after a successful submit). */
  clear: () => void;
}

export function useFormValidation<T>(
  values: T,
  rules: Partial<Record<keyof T, ValidationRule<T>>>
): FormValidationHandle<T> {
  const [errors, setErrors] = useState<Partial<Record<keyof T, string>>>({});

  const computeErrors = useCallback((): Partial<Record<keyof T, string>> => {
    const next: Partial<Record<keyof T, string>> = {};
    for (const key of Object.keys(rules) as (keyof T)[]) {
      const rule = rules[key];
      if (!rule) continue;
      const msg = rule(values);
      if (msg) next[key] = msg;
    }
    return next;
  }, [values, rules]);

  const isValid = useMemo(() => {
    const computed = computeErrors();
    return Object.keys(computed).length === 0;
  }, [computeErrors]);

  const validate = useCallback(() => {
    const next = computeErrors();
    setErrors(next);
    return Object.keys(next).length === 0;
  }, [computeErrors]);

  const clear = useCallback(() => {
    setErrors({});
  }, []);

  return { errors, isValid, validate, clear };
}
