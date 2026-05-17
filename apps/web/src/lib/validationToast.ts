// Toast surface for `CoreError::InvalidStreamConfig` / `ValidationFailed`
// payloads. The reasons array comes straight from the wire (see
// `packages/types/src/generated/ValidationIssue.ts`); this helper just
// flattens up to three issues into toasts and rolls the rest into a count.

import i18n from '@/lib/i18n';
import type { ValidationIssue } from '@spiritstream/types';
import type { StreamValidationFailure } from '@spiritstream/api-client';

type ToastFn = {
  error: (msg: string) => void;
  info: (msg: string) => void;
};

export function displayValidationIssues(issues: ValidationIssue[], toastFn: ToastFn): void {
  const displayCount = Math.min(issues.length, 3);
  for (let i = 0; i < displayCount; i++) {
    toastFn.error(issues[i].message);
  }
  if (issues.length > 3) {
    toastFn.info(i18n.t('errors.moreIssues', { count: issues.length - 3 }));
  }
}

/**
 * Convenience: pull the `reasons` array out of a thrown
 * `StreamValidationFailure` (or any structured error from `api.stream.validate`
 * / `api.profile.save`) and surface it as toasts. Falls back to a single
 * toast carrying the error's message when the error isn't structured.
 */
export function displayValidationError(err: unknown, toastFn: ToastFn): void {
  const e = err as StreamValidationFailure;
  const reasons = e?.details?.reasons;
  if (reasons && reasons.length > 0) {
    displayValidationIssues(reasons, toastFn);
    return;
  }
  toastFn.error(err instanceof Error ? err.message : String(err));
}
