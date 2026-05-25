import type { RotationReport, Settings as AppSettings } from '@spiritstream/types';
import { fetchTypedJson, withConfirmToken } from './_internal';

export const settings = {
  get: () => fetchTypedJson<AppSettings>('GET', '/api/v1/settings'),
  save: async (settings: AppSettings) => {
    await fetchTypedJson<{ saved: boolean }>('PUT', '/api/v1/settings', undefined, { settings });
  },
  getProfilesPath: async () => {
    const { path } = await fetchTypedJson<{ path: string }>(
      'GET',
      '/api/v1/settings/profiles-path',
    );
    return path;
  },
  exportData: async (exportPath: string) => {
    await fetchTypedJson<{ exported: boolean }>('POST', '/api/v1/settings/export', undefined, {
      exportPath,
    });
  },
  clearData: async () => {
    // Destructive op gated by one-shot confirm token.
    // The backend (`DELETE /api/v1/settings/data` at
    // crates/transport-http/src/v1.rs::v1_settings_clear_data) calls
    // `require_confirm_token(state, headers, "clear_data")` which
    // rejects requests missing `X-Confirm-Token`. We acquire the
    // token + attach in one helper call.
    await withConfirmToken<{ cleared: boolean }>('clear_data', (headers) =>
      fetchTypedJson('DELETE', '/api/v1/settings/data', undefined, undefined, headers),
    );
  },
  rotateMachineKey: (unlockedPasswords: Record<string, string> = {}) =>
    withConfirmToken<RotationReport>('rotate_machine_key', (headers) =>
      fetchTypedJson(
        'POST',
        '/api/v1/security/machine-key/rotate',
        undefined,
        { unlockedPasswords },
        headers,
      ),
    ),
};
