import { fetchTypedJson } from './_internal';

export const discord = {
  testWebhook: (url: string) =>
    fetchTypedJson<{ success: boolean; message: string; skippedCooldown: boolean }>(
      'POST',
      '/api/v1/discord/webhook/test',
      undefined,
      { url },
    ),
  sendNotification: () =>
    fetchTypedJson<{ success: boolean; message: string; skippedCooldown: boolean }>(
      'POST',
      '/api/v1/discord/webhook/send',
    ),
  resetCooldown: async () => {
    await fetchTypedJson<unknown>('DELETE', '/api/v1/discord/webhook/cooldown');
  },
};
