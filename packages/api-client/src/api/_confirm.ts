/**
 * Confirm-token helper for destructive operations, over the generated client.
 *
 * Destructive endpoints (`clear_data`, `rotate_machine_key`,
 * `revoke_all_sessions`) require a one-shot, intent-scoped token attached as
 * `X-Confirm-Token`. This issues the token and returns the header object to
 * spread into the destructive call's `headers` option:
 *
 *   await v1SettingsClearData({ headers: await confirmTokenHeader('clear_data'), throwOnError: true });
 */
import { confirmTokenIssue } from '../generated';

export async function confirmTokenHeader(intent: string): Promise<Record<string, string>> {
  const { data } = await confirmTokenIssue({ body: { intent }, throwOnError: true });
  return { 'X-Confirm-Token': data.token };
}
