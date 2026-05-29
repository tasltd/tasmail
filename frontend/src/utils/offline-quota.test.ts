import { describe, it, expect, afterEach, vi } from 'vitest';
import {
  getQuotaEstimate,
  isLowOnSpace,
  shouldRejectWrite,
  requestPersistentStorage,
  MAX_CACHE_BYTES,
  HIGH_USAGE_RATIO,
} from './offline-quota';

// jsdom does not implement navigator.storage by default — we stub it per test.
function stubStorage(partial: Partial<StorageManager>) {
  Object.defineProperty(navigator, 'storage', {
    configurable: true,
    value: partial as StorageManager,
  });
}

afterEach(() => {
  vi.restoreAllMocks();
  // Remove our stub so the next test starts clean
  Object.defineProperty(navigator, 'storage', {
    configurable: true,
    value: undefined,
  });
});

describe('offline-quota (TMAIL-87)', () => {
  describe('getQuotaEstimate', () => {
    it('returns unsupported=false when navigator.storage is missing', async () => {
      const est = await getQuotaEstimate();
      expect(est.supported).toBe(false);
      expect(est.usage).toBe(0);
      expect(est.ratio).toBe(0);
    });

    it('computes ratio from the StorageManager estimate', async () => {
      stubStorage({
        estimate: async () => ({ usage: 25, quota: 100 }),
      });
      const est = await getQuotaEstimate();
      expect(est.supported).toBe(true);
      expect(est.usage).toBe(25);
      expect(est.quota).toBe(100);
      expect(est.ratio).toBe(0.25);
    });

    it('treats quota=0 as ratio=0 to avoid NaN', async () => {
      stubStorage({
        estimate: async () => ({ usage: 10, quota: 0 }),
      });
      const est = await getQuotaEstimate();
      expect(est.ratio).toBe(0);
    });
  });

  describe('isLowOnSpace', () => {
    it('returns false when StorageManager is unsupported', async () => {
      expect(await isLowOnSpace()).toBe(false);
    });

    it('returns false when usage is well below threshold', async () => {
      stubStorage({ estimate: async () => ({ usage: 10, quota: 100 }) });
      expect(await isLowOnSpace()).toBe(false);
    });

    it('returns true when usage meets the default threshold', async () => {
      stubStorage({ estimate: async () => ({ usage: 81, quota: 100 }) });
      expect(await isLowOnSpace()).toBe(true);
    });

    it('honors a custom threshold', async () => {
      stubStorage({ estimate: async () => ({ usage: 60, quota: 100 }) });
      expect(await isLowOnSpace(0.5)).toBe(true);
      expect(await isLowOnSpace(0.9)).toBe(false);
    });
  });

  describe('shouldRejectWrite', () => {
    it('rejects when current cache bytes exceed soft cap', async () => {
      expect(await shouldRejectWrite(MAX_CACHE_BYTES + 1)).toBe(true);
    });

    it('rejects when low on space even if below soft cap', async () => {
      stubStorage({ estimate: async () => ({ usage: 95, quota: 100 }) });
      expect(await shouldRejectWrite(0)).toBe(true);
    });

    it('allows writes when cache is small and quota is healthy', async () => {
      stubStorage({ estimate: async () => ({ usage: 1, quota: 100 }) });
      expect(await shouldRejectWrite(1024)).toBe(false);
    });
  });

  describe('requestPersistentStorage', () => {
    it('returns false when StorageManager.persist is missing', async () => {
      expect(await requestPersistentStorage()).toBe(false);
    });

    it('returns the StorageManager.persist result when supported', async () => {
      const persist = vi.fn(async () => true);
      stubStorage({ persist });
      expect(await requestPersistentStorage()).toBe(true);
      expect(persist).toHaveBeenCalled();
    });

    it('returns false if persist throws', async () => {
      stubStorage({
        persist: async () => {
          throw new Error('denied');
        },
      });
      expect(await requestPersistentStorage()).toBe(false);
    });
  });

  describe('constants', () => {
    it('exposes a sensible soft cap and threshold', () => {
      expect(MAX_CACHE_BYTES).toBeGreaterThan(0);
      expect(HIGH_USAGE_RATIO).toBeGreaterThan(0);
      expect(HIGH_USAGE_RATIO).toBeLessThanOrEqual(1);
    });
  });
});
