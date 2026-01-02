import { api } from '@/lib/tauri';
import type { Profile } from '@/types/profile';

/**
 * Validation issue with actionable message
 */
export interface ValidationIssue {
  code: string;
  message: string;
  severity: 'error' | 'warning';
  field?: string;
}

/**
 * Result of stream configuration validation
 */
export interface ValidationResult {
  valid: boolean;
  issues: ValidationIssue[];
}

/**
 * Options for validation behavior
 */
export interface ValidationOptions {
  /** Check if FFmpeg is available (default: true) */
  checkFfmpeg?: boolean;
  /** Only validate enabled targets, not all targets (default: false) */
  checkEnabledTargetsOnly?: boolean;
  /** Set of target IDs that are enabled (required if checkEnabledTargetsOnly is true) */
  enabledTargetIds?: Set<string>;
}

/**
 * Validates a streaming profile configuration.
 * Returns all issues found, not just the first one.
 *
 * Validation order:
 * 1. FFmpeg availability (async)
 * 2. Incoming URL configured
 * 3. Output groups exist
 * 4. Stream targets exist
 * 5. Each target has valid URL
 * 6. Each target has stream key
 * 7. Each output group has video encoder
 * 8. Each output group has resolution
 */
export async function validateStreamConfig(
  profile: Profile,
  options: ValidationOptions = {}
): Promise<ValidationResult> {
  const issues: ValidationIssue[] = [];
  const {
    checkFfmpeg = true,
    checkEnabledTargetsOnly = false,
    enabledTargetIds = new Set<string>(),
  } = options;

  // 1. FFmpeg availability (async - do this first as it's the most critical)
  if (checkFfmpeg) {
    try {
      const ffmpegVersion = await api.system.testFfmpeg();
      if (!ffmpegVersion || ffmpegVersion.includes('not found')) {
        issues.push({
          code: 'FFMPEG_NOT_FOUND',
          message: 'FFmpeg not found. Please install FFmpeg or set the path in Settings.',
          severity: 'error',
        });
      }
    } catch {
      issues.push({
        code: 'FFMPEG_UNAVAILABLE',
        message: 'FFmpeg not available. Please install FFmpeg.',
        severity: 'error',
      });
    }
  }

  // 2. Incoming URL
  if (!profile.incomingUrl || profile.incomingUrl.trim() === '') {
    issues.push({
      code: 'MISSING_INCOMING_URL',
      message: 'No incoming URL configured. Set an RTMP source URL in your profile.',
      severity: 'error',
      field: 'incomingUrl',
    });
  }

  // 3. Output groups exist
  if (profile.outputGroups.length === 0) {
    issues.push({
      code: 'NO_OUTPUT_GROUPS',
      message: 'No output groups configured.',
      severity: 'error',
    });
  }

  // 4. Get targets to validate (optionally filtered by enabled)
  const allTargets = profile.outputGroups.flatMap(g => g.streamTargets);
  const targetsToValidate = checkEnabledTargetsOnly
    ? allTargets.filter(t => enabledTargetIds.has(t.id))
    : allTargets;

  // 5. At least one target exists
  if (targetsToValidate.length === 0) {
    issues.push({
      code: 'NO_STREAM_TARGETS',
      message: checkEnabledTargetsOnly
        ? 'No stream targets enabled. Enable at least one destination.'
        : 'No stream targets configured. Add at least one destination.',
      severity: 'error',
    });
  }

  // 6. Validate each target
  for (const target of targetsToValidate) {
    const targetName = target.name || 'Unnamed';

    if (!target.url || target.url.trim() === '') {
      issues.push({
        code: 'TARGET_MISSING_URL',
        message: `Target "${targetName}" has no URL configured.`,
        severity: 'error',
        field: `target.${target.id}.url`,
      });
    }

    if (!target.streamKey || target.streamKey.trim() === '') {
      issues.push({
        code: 'TARGET_MISSING_STREAM_KEY',
        message: `Target "${targetName}" has no stream key configured.`,
        severity: 'error',
        field: `target.${target.id}.streamKey`,
      });
    }
  }

  // 7. Validate each output group's encoder settings
  for (const group of profile.outputGroups) {
    const groupName = group.name || 'Unnamed';

    // Only validate groups that have enabled targets (if filtering)
    if (checkEnabledTargetsOnly) {
      const hasEnabledTargets = group.streamTargets.some(t => enabledTargetIds.has(t.id));
      if (!hasEnabledTargets) continue;
    }

    if (!group.videoEncoder || group.videoEncoder.trim() === '') {
      issues.push({
        code: 'GROUP_MISSING_ENCODER',
        message: `Output group "${groupName}" has no video encoder set.`,
        severity: 'error',
        field: `group.${group.id}.videoEncoder`,
      });
    }

    if (!group.resolution || group.resolution.trim() === '') {
      issues.push({
        code: 'GROUP_MISSING_RESOLUTION',
        message: `Output group "${groupName}" has no resolution set.`,
        severity: 'error',
        field: `group.${group.id}.resolution`,
      });
    }
  }

  return {
    valid: issues.filter(i => i.severity === 'error').length === 0,
    issues,
  };
}

/**
 * Display validation issues as toast notifications.
 * Shows up to 3 issues, then a summary if more exist.
 */
export function displayValidationIssues(
  issues: ValidationIssue[],
  toastFn: {
    error: (msg: string) => void;
    info: (msg: string) => void;
  }
): void {
  const errors = issues.filter(i => i.severity === 'error');
  const displayCount = Math.min(errors.length, 3);

  for (let i = 0; i < displayCount; i++) {
    toastFn.error(errors[i].message);
  }

  if (errors.length > 3) {
    toastFn.info(`...and ${errors.length - 3} more issues to fix`);
  }
}
