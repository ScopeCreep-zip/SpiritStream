// Per-target RTMP connectivity testing. Pure orchestration — each
// target's test is one backend call to `api.system.testRtmpTarget`, and
// this file just iterates and aggregates. The actual connectivity logic
// lives in core.
//
// `streamValidation.ts` was deleted — config validation
// moved to `api.stream.validate()` (backed by `POST /api/v1/streams/validate`).
// Connectivity testing stayed on the frontend because it's a UI affordance
// (sequential, progress-reported, cancelable from the user's perspective)
// rather than a config rule.

import { api } from '@/lib/client';
import type { Profile, StreamTarget, RtmpTestResult } from '@spiritstream/types';

export interface TargetTestResult {
  targetId: string;
  targetName: string;
  success: boolean;
  message: string;
  latencyMs: number | null;
}

export interface ConnectivityTestResult {
  allPassed: boolean;
  results: TargetTestResult[];
  passed: number;
  failed: number;
}

export type ConnectivityProgressCallback = (
  current: number,
  total: number,
  targetName: string,
  result?: TargetTestResult
) => void;

export async function testStreamConnectivity(
  profile: Profile,
  options: {
    enabledTargetsOnly?: boolean;
    enabledTargetIds?: Set<string>;
  } = {},
  onProgress?: ConnectivityProgressCallback
): Promise<ConnectivityTestResult> {
  const { enabledTargetsOnly = false, enabledTargetIds = new Set<string>() } = options;

  const allTargets: Array<{ target: StreamTarget; groupName: string }> = [];
  for (const group of profile.outputGroups) {
    for (const target of group.streamTargets) {
      if (enabledTargetsOnly && !enabledTargetIds.has(target.id)) continue;
      allTargets.push({ target, groupName: group.name });
    }
  }

  if (allTargets.length === 0) {
    return { allPassed: false, results: [], passed: 0, failed: 0 };
  }

  const results: TargetTestResult[] = [];
  let passed = 0;
  let failed = 0;

  for (let i = 0; i < allTargets.length; i++) {
    const { target, groupName } = allTargets[i];
    const targetName = target.name || `${groupName} - Unnamed`;
    onProgress?.(i + 1, allTargets.length, targetName);

    try {
      const result: RtmpTestResult = await api.system.testRtmpTarget(target.url, target.streamKey);
      const testResult: TargetTestResult = {
        targetId: target.id,
        targetName,
        success: result.success,
        message: result.message,
        latencyMs:
          result.latencyMs === null || result.latencyMs === undefined
            ? null
            : Number(result.latencyMs),
      };
      results.push(testResult);
      if (result.success) passed++;
      else failed++;
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

  return { allPassed: failed === 0 && passed > 0, results, passed, failed };
}
