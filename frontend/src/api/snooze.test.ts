import { describe, it, expect, vi, beforeEach } from 'vitest';
import { snoozeMessage, listSnoozed, cancelSnooze, getSnoozePresets } from './snooze';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('snooze API', () => {
  beforeEach(() => vi.clearAllMocks());

  it('snoozes a message', async () => {
    const mockSnooze = { id: 's1', folder: 'INBOX', message_uid: 42, snooze_until: '2026-04-15T09:00:00Z' };
    vi.mocked(apiClient.post).mockResolvedValue(mockSnooze);
    const result = await snoozeMessage({ folder: 'INBOX', message_uid: 42, snooze_until: '2026-04-15T09:00:00Z' });
    expect(apiClient.post).toHaveBeenCalledWith('/messages/snooze', expect.objectContaining({ message_uid: 42 }));
    expect(result.id).toBe('s1');
  });

  it('lists snoozed emails', async () => {
    vi.mocked(apiClient.get).mockResolvedValue([]);
    const result = await listSnoozed();
    expect(apiClient.get).toHaveBeenCalledWith('/messages/snoozed');
    expect(result).toHaveLength(0);
  });

  it('cancels a snooze', async () => {
    vi.mocked(apiClient.delete).mockResolvedValue(undefined);
    await cancelSnooze('s1');
    expect(apiClient.delete).toHaveBeenCalledWith('/messages/snooze/s1');
  });
});

describe('snooze presets', () => {
  it('returns 3 presets', () => {
    const presets = getSnoozePresets();
    expect(presets).toHaveLength(3);
    expect(presets[0].label).toBe('Later today');
    expect(presets[1].label).toBe('Tomorrow morning');
    expect(presets[2].label).toBe('Next week');
  });

  it('all presets produce future dates', () => {
    const now = new Date();
    const presets = getSnoozePresets();
    for (const preset of presets) {
      expect(preset.getTime().getTime()).toBeGreaterThan(now.getTime());
    }
  });

  it('later today is within 4 hours', () => {
    const now = new Date();
    const laterToday = getSnoozePresets()[0].getTime();
    const diffHours = (laterToday.getTime() - now.getTime()) / (1000 * 60 * 60);
    expect(diffHours).toBeGreaterThan(2.5);
    expect(diffHours).toBeLessThan(4);
  });
});
