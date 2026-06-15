import type { RotationReport, Settings as AppSettings } from '@spiritstream/types';
import {
  v1SettingsGet,
  v1SettingsSave,
  v1SettingsProfilesPath,
  v1SettingsExport,
  v1SettingsClearData,
  v1SecurityRotateMachineKeyProxy,
} from '../generated';
import { confirmTokenHeader } from './_confirm';

export const settings = {
  get: async (): Promise<AppSettings> => {
    const { data } = await v1SettingsGet({ throwOnError: true });
    return data as AppSettings;
  },
  save: async (settings: AppSettings): Promise<void> => {
    await v1SettingsSave({ body: { settings }, throwOnError: true });
  },
  getProfilesPath: async (): Promise<string> => {
    const { data } = await v1SettingsProfilesPath({ throwOnError: true });
    return data.path;
  },
  exportData: async (exportPath: string): Promise<void> => {
    await v1SettingsExport({ body: { exportPath }, throwOnError: true });
  },
  clearData: async (): Promise<void> => {
    // Destructive op gated by a one-shot confirm token. The backend
    // (`DELETE /api/v1/settings/data`) calls `require_confirm_token(..,
    // "clear_data")` and rejects requests missing `X-Confirm-Token`.
    await v1SettingsClearData({
      headers: await confirmTokenHeader('clear_data'),
      throwOnError: true,
    });
  },
  rotateMachineKey: async (
    unlockedPasswords: Record<string, string> = {}
  ): Promise<RotationReport> => {
    const { data } = await v1SecurityRotateMachineKeyProxy({
      headers: await confirmTokenHeader('rotate_machine_key'),
      body: { unlockedPasswords },
      throwOnError: true,
    });
    return data as RotationReport;
  },
};
