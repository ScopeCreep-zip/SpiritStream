import { describe, it, expect, beforeEach } from 'vitest';
import { useSettingsStore } from './settingsStore';

// N5: global notification-preference store. Small, but it gates whether
// the stream store fires desktop notifications (see stream/stats.ts), so
// its default and setter are worth pinning — a flipped default would
// silently notify users who opted out.

beforeEach(() => {
  useSettingsStore.setState({ showNotifications: true });
});

describe('settingsStore', () => {
  it('defaults showNotifications to true', () => {
    expect(useSettingsStore.getState().showNotifications).toBe(true);
  });

  it('setShowNotifications toggles the flag', () => {
    useSettingsStore.getState().setShowNotifications(false);
    expect(useSettingsStore.getState().showNotifications).toBe(false);
    useSettingsStore.getState().setShowNotifications(true);
    expect(useSettingsStore.getState().showNotifications).toBe(true);
  });
});
