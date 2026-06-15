import {
  v1DiscordTestWebhookProxy,
  v1DiscordSendNotificationProxy,
  v1DiscordResetCooldownProxy,
} from '../generated';

export const discord = {
  testWebhook: async (url: string) => {
    const { data } = await v1DiscordTestWebhookProxy({ body: { url }, throwOnError: true });
    return data;
  },
  sendNotification: async () => {
    const { data } = await v1DiscordSendNotificationProxy({ throwOnError: true });
    return data;
  },
  resetCooldown: async (): Promise<void> => {
    await v1DiscordResetCooldownProxy({ throwOnError: true });
  },
};
