/**
 * Storage quota helpers for the offline email cache (TMAIL-87).
 *
 * Browsers grant a finite slice of disk to a given origin (often 60% of free
 * disk, soft-capped per browser). When the cache approaches that limit the
 * write will throw `QuotaExceededError` and the user loses data. We monitor
 * usage via `navigator.storage.estimate()` and prune proactively before that
 * happens.
 */

export interface QuotaStatus {
  usage: number;       // bytes used by this origin
  quota: number;       // bytes available to this origin
  ratio: number;       // usage / quota, 0..1
  supported: boolean;  // false on older browsers without StorageManager
}

// Soft cap: TASMail will not knowingly grow its cache beyond this many bytes
// even if the browser would allow more. Keeps headroom for other origins on
// shared devices and avoids surprise eviction on disk-pressure.
export const MAX_CACHE_BYTES = 50 * 1024 * 1024; // 50 MB

// Warn / prune when origin-wide usage exceeds this fraction of the quota.
export const HIGH_USAGE_RATIO = 0.8;

// Added: Wrap navigator.storage.estimate() with a typed fallback for jsdom.
export async function getQuotaEstimate(): Promise<QuotaStatus> {
  if (
    typeof navigator === 'undefined' ||
    !navigator.storage ||
    typeof navigator.storage.estimate !== 'function'
  ) {
    return { usage: 0, quota: 0, ratio: 0, supported: false };
  }
  const est = await navigator.storage.estimate();
  const usage = est.usage ?? 0;
  const quota = est.quota ?? 0;
  const ratio = quota > 0 ? usage / quota : 0;
  return { usage, quota, ratio, supported: true };
}

// Added: True when the origin is using more than HIGH_USAGE_RATIO of its quota.
export async function isLowOnSpace(threshold = HIGH_USAGE_RATIO): Promise<boolean> {
  const { supported, ratio } = await getQuotaEstimate();
  if (!supported) return false;
  return ratio >= threshold;
}

// Added: True when the cache should refuse new writes (origin near quota OR over our soft cap).
export async function shouldRejectWrite(currentCacheBytes: number): Promise<boolean> {
  if (currentCacheBytes >= MAX_CACHE_BYTES) return true;
  return isLowOnSpace();
}

// Added: Best-effort persistent storage request. When granted, the browser will
// not evict this origin under disk pressure — important for offline-first apps.
export async function requestPersistentStorage(): Promise<boolean> {
  if (
    typeof navigator === 'undefined' ||
    !navigator.storage ||
    typeof navigator.storage.persist !== 'function'
  ) {
    return false;
  }
  try {
    return await navigator.storage.persist();
  } catch {
    return false;
  }
}
