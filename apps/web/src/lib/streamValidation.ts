import { api } from '@/lib/backend';
import i18n from '@/lib/i18n';
import type { Profile, StreamTarget } from '@/types/profile';
import type { RtmpTestResult } from '@/types/api';

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
          message: i18n.t('errors.ffmpegNotFound'),
          severity: 'error',
        });
      }
    } catch {
      issues.push({
        code: 'FFMPEG_UNAVAILABLE',
        message: i18n.t('errors.ffmpegNotAvailable'),
        severity: 'error',
      });
    }
  }

  // 2. RTMP Input configuration
  if (!profile.input || !profile.input.bindAddress || !profile.input.application) {
    issues.push({
      code: 'MISSING_INCOMING_URL',
      message: i18n.t('errors.noIncomingUrl'),
      severity: 'error',
      field: 'input',
    });
  }

  // 3. Output groups exist
  if (profile.outputGroups.length === 0) {
    issues.push({
      code: 'NO_OUTPUT_GROUPS',
      message: i18n.t('errors.noOutputGroups'),
      severity: 'error',
    });
  }

  // 4. Get targets to validate (optionally filtered by enabled)
  const allTargets = profile.outputGroups.flatMap((g) => g.streamTargets);
  const targetsToValidate = checkEnabledTargetsOnly
    ? allTargets.filter((t) => enabledTargetIds.has(t.id))
    : allTargets;

  // 5. At least one target exists
  if (targetsToValidate.length === 0) {
    issues.push({
      code: 'NO_STREAM_TARGETS',
      message: checkEnabledTargetsOnly
        ? i18n.t('errors.noTargetsEnabled')
        : i18n.t('errors.noTargetsConfigured'),
      severity: 'error',
    });
  }

  // 6. Validate each target
  for (const target of targetsToValidate) {
    const targetName = target.name || i18n.t('common.unnamed', 'Unnamed');

    if (!target.url || target.url.trim() === '') {
      issues.push({
        code: 'TARGET_MISSING_URL',
        message: i18n.t('errors.targetMissingUrl', { name: targetName }),
        severity: 'error',
        field: `target.${target.id}.url`,
      });
    }

    if (!target.streamKey || target.streamKey.trim() === '') {
      issues.push({
        code: 'TARGET_MISSING_STREAM_KEY',
        message: i18n.t('errors.targetMissingKey', { name: targetName }),
        severity: 'error',
        field: `target.${target.id}.streamKey`,
      });
    }
  }

  // 7. Validate each output group's encoder settings
  for (const group of profile.outputGroups) {
    const groupName = group.name || i18n.t('common.unnamed', 'Unnamed');

    // Only validate groups that have enabled targets (if filtering)
    if (checkEnabledTargetsOnly) {
      const hasEnabledTargets = group.streamTargets.some((t) => enabledTargetIds.has(t.id));
      if (!hasEnabledTargets) continue;
    }

    if (!group.video?.codec || group.video.codec.trim() === '') {
      issues.push({
        code: 'GROUP_MISSING_ENCODER',
        message: i18n.t('errors.groupMissingEncoder', { name: groupName }),
        severity: 'error',
        field: `group.${group.id}.video.codec`,
      });
    }

    // Skip resolution validation for passthrough mode (codec: "copy")
    // Passthrough mode doesn't re-encode, so resolution is not needed
    const isPassthrough = group.video?.codec === 'copy' && group.audio?.codec === 'copy';
    if (!isPassthrough && (!group.video?.width || !group.video?.height)) {
      issues.push({
        code: 'GROUP_MISSING_RESOLUTION',
        message: i18n.t('errors.groupMissingResolution', { name: groupName }),
        severity: 'error',
        field: `group.${group.id}.video.resolution`,
      });
    }
  }

  return {
    valid: issues.filter((i) => i.severity === 'error').length === 0,
    issues,
  };
}

/**
 * Result of testing a single target's connectivity
 */
export interface TargetTestResult {
  targetId: string;
  targetName: string;
  success: boolean;
  message: string;
  latencyMs: number | null;
}

/**
 * Result of testing all targets' connectivity
 */
export interface ConnectivityTestResult {
  allPassed: boolean;
  results: TargetTestResult[];
  /** Targets that passed */
  passed: number;
  /** Targets that failed */
  failed: number;
}

/**
 * Progress callback for connectivity testing
 */
export type ConnectivityProgressCallback = (
  current: number,
  total: number,
  targetName: string,
  result?: TargetTestResult
) => void;

/**
 * Test actual RTMP connectivity for all targets in a profile.
 * This performs real connection attempts to each streaming platform.
 *
 * @param profile The profile to test
 * @param options Test options
 * @param onProgress Optional callback for progress updates
 */
export async function testStreamConnectivity(
  profile: Profile,
  options: {
    /** Only test enabled targets (default: false = test all) */
    enabledTargetsOnly?: boolean;
    /** Set of target IDs that are enabled */
    enabledTargetIds?: Set<string>;
  } = {},
  onProgress?: ConnectivityProgressCallback
): Promise<ConnectivityTestResult> {
  const { enabledTargetsOnly = false, enabledTargetIds = new Set<string>() } = options;

  // Collect all targets to test
  const allTargets: Array<{ target: StreamTarget; groupName: string }> = [];
  for (const group of profile.outputGroups) {
    for (const target of group.streamTargets) {
      if (enabledTargetsOnly && !enabledTargetIds.has(target.id)) {
        continue;
      }
      allTargets.push({ target, groupName: group.name });
    }
  }

  if (allTargets.length === 0) {
    return {
      allPassed: false,
      results: [],
      passed: 0,
      failed: 0,
    };
  }

  const results: TargetTestResult[] = [];
  let passed = 0;
  let failed = 0;

  // Test each target sequentially (to avoid overwhelming the network)
  for (let i = 0; i < allTargets.length; i++) {
    const { target, groupName } = allTargets[i];
    const targetName = target.name || `${groupName} - Unnamed`;

    // Report progress before testing
    onProgress?.(i + 1, allTargets.length, targetName);

    try {
      const result: RtmpTestResult = await api.system.testRtmpTarget(target.url, target.streamKey);

      const testResult: TargetTestResult = {
        targetId: target.id,
        targetName,
        success: result.success,
        message: result.message,
        latencyMs: result.latency_ms,
      };

      results.push(testResult);

      if (result.success) {
        passed++;
      } else {
        failed++;
      }

      // Report progress after testing
      onProgress?.(i + 1, allTargets.length, targetName, testResult);
    } catch (err) {
      const testResult: TargetTestResult = {
        targetId: target.id,
        targetName,
        success: false,
        message: err instanceof Error ? err.message : 'Unknown error',
        latencyMs: null,
      };

      results.push(testResult);
      failed++;

      onProgress?.(i + 1, allTargets.length, targetName, testResult);
    }
  }

  return {
    allPassed: failed === 0 && passed > 0,
    results,
    passed,
    failed,
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
  const errors = issues.filter((i) => i.severity === 'error');
  const displayCount = Math.min(errors.length, 3);

  for (let i = 0; i < displayCount; i++) {
    toastFn.error(errors[i].message);
  }

  if (errors.length > 3) {
    toastFn.info(i18n.t('errors.moreIssues', { count: errors.length - 3 }));
  }
}
