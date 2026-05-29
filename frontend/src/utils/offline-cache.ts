/**
 * IndexedDB-based offline email cache for PWA support (TMAIL-87).
 *
 * Stores:
 *   - `folders`         — folder list (5min freshness TTL)
 *   - `messages`        — paginated envelope lists per folder (2min TTL)
 *   - `fullMessages`    — full message bodies, plaintext (legacy; 30min TTL)
 *   - `emails`          — full message bodies, AES-256-GCM encrypted at rest, 30d retention
 *   - `drafts`          — offline compose drafts, AES-256-GCM encrypted at rest
 *
 * Retention: emails are kept for {@link EMAIL_RETENTION_DAYS} days then pruned.
 * Quota: writes check `navigator.storage.estimate()` and prune proactively
 * (see `offline-quota.ts`).
 *
 * Sync queue (`pending-actions` store) lives in `background-sync.ts`.
 */

import { encryptJson, decryptJson, type EncryptedEnvelope } from './offline-encryption';
import { isLowOnSpace, MAX_CACHE_BYTES } from './offline-quota';

const DB_NAME = 'tasmail-cache';
// Bumped to 2 for TMAIL-87: adds `emails` + `drafts` stores with cachedAt indexes for pruning.
const DB_VERSION = 2;

// Default retention window for offline-cached emails (configurable per call).
export const EMAIL_RETENTION_DAYS = 30;

interface CacheEntry<T> {
  key: string;
  data: T;
  cachedAt: number;
}

// Encrypted cache entry stored in the `emails` / `drafts` stores. The body
// payload is opaque ciphertext; everything else is searchable metadata so the
// pruner can sweep without decrypting every row.
interface EncryptedCacheEntry {
  key: string;
  cachedAt: number;
  receivedAt?: number;  // for emails: original message Date — drives retention
  size: number;         // approximate ciphertext size in bytes
  envelope: EncryptedEnvelope;
}

function openDB(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);

    request.onupgradeneeded = (event) => {
      const db = request.result;
      const oldVersion = event.oldVersion;

      if (!db.objectStoreNames.contains('folders')) {
        db.createObjectStore('folders', { keyPath: 'key' });
      }
      if (!db.objectStoreNames.contains('messages')) {
        db.createObjectStore('messages', { keyPath: 'key' });
      }
      if (!db.objectStoreNames.contains('fullMessages')) {
        db.createObjectStore('fullMessages', { keyPath: 'key' });
      }
      // Added (TMAIL-87): encrypted emails store with cachedAt index for fast prune scans.
      if (oldVersion < 2) {
        if (!db.objectStoreNames.contains('emails')) {
          const store = db.createObjectStore('emails', { keyPath: 'key' });
          store.createIndex('cachedAt', 'cachedAt', { unique: false });
          store.createIndex('receivedAt', 'receivedAt', { unique: false });
        }
        if (!db.objectStoreNames.contains('drafts')) {
          const store = db.createObjectStore('drafts', { keyPath: 'key' });
          store.createIndex('cachedAt', 'cachedAt', { unique: false });
        }
      }
    };

    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

async function put<T>(storeName: string, key: string, data: T): Promise<void> {
  const db = await openDB();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(storeName, 'readwrite');
    const store = tx.objectStore(storeName);
    const entry: CacheEntry<T> = { key, data, cachedAt: Date.now() };
    store.put(entry);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

async function get<T>(storeName: string, key: string, maxAgeMs?: number): Promise<T | null> {
  const db = await openDB();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(storeName, 'readonly');
    const store = tx.objectStore(storeName);
    const request = store.get(key);
    request.onsuccess = () => {
      const entry = request.result as CacheEntry<T> | undefined;
      if (!entry) return resolve(null);
      if (maxAgeMs && Date.now() - entry.cachedAt > maxAgeMs) return resolve(null);
      resolve(entry.data);
    };
    request.onerror = () => reject(request.error);
  });
}

// Added (TMAIL-87): write an encrypted envelope into the given store.
async function putEncrypted(
  storeName: 'emails' | 'drafts',
  key: string,
  envelope: EncryptedEnvelope,
  receivedAt?: number,
): Promise<void> {
  const db = await openDB();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(storeName, 'readwrite');
    const entry: EncryptedCacheEntry = {
      key,
      cachedAt: Date.now(),
      receivedAt,
      size: envelope.ciphertext.byteLength + envelope.iv.byteLength,
      envelope,
    };
    tx.objectStore(storeName).put(entry);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

async function getEncryptedEntry(
  storeName: 'emails' | 'drafts',
  key: string,
): Promise<EncryptedCacheEntry | null> {
  const db = await openDB();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(storeName, 'readonly');
    const req = tx.objectStore(storeName).get(key);
    req.onsuccess = () => resolve((req.result as EncryptedCacheEntry | undefined) ?? null);
    req.onerror = () => reject(req.error);
  });
}

// Added (TMAIL-87): iterate a store and call cb for each entry, deleting when cb returns true.
async function pruneStore(
  storeName: 'emails' | 'drafts' | 'fullMessages' | 'messages' | 'folders',
  shouldDelete: (entry: { key: string; cachedAt: number; receivedAt?: number; size?: number }) => boolean,
): Promise<number> {
  const db = await openDB();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(storeName, 'readwrite');
    const store = tx.objectStore(storeName);
    const req = store.openCursor();
    let deleted = 0;
    req.onsuccess = () => {
      const cursor = req.result;
      if (!cursor) return;
      const entry = cursor.value as { key: string; cachedAt: number; receivedAt?: number; size?: number };
      if (shouldDelete(entry)) {
        cursor.delete();
        deleted += 1;
      }
      cursor.continue();
    };
    tx.oncomplete = () => resolve(deleted);
    tx.onerror = () => reject(tx.error);
  });
}

// Added (TMAIL-87): sum size field across the emails store — used by quota guard.
async function getEmailsCacheBytes(): Promise<number> {
  const db = await openDB();
  return new Promise((resolve, reject) => {
    const tx = db.transaction('emails', 'readonly');
    const req = tx.objectStore('emails').openCursor();
    let total = 0;
    req.onsuccess = () => {
      const cursor = req.result;
      if (!cursor) return;
      total += (cursor.value as EncryptedCacheEntry).size ?? 0;
      cursor.continue();
    };
    tx.oncomplete = () => resolve(total);
    tx.onerror = () => reject(tx.error);
  });
}

export interface CachedEmail<T = unknown> {
  data: T;
  cachedAt: number;
  receivedAt?: number;
}

export interface CachedDraft<T = unknown> {
  data: T;
  cachedAt: number;
}

export const offlineCache = {
  // Folders cache (5 min TTL)
  cacheFolders: (folders: unknown) => put('folders', 'all', folders),
  getFolders: () => get<unknown>('folders', 'all', 5 * 60 * 1000),

  // Message list cache per folder (2 min TTL)
  cacheMessages: (folder: string, page: number, data: unknown) =>
    put('messages', `${folder}:${page}`, data),
  getMessages: (folder: string, page: number) =>
    get<unknown>('messages', `${folder}:${page}`, 2 * 60 * 1000),

  // Full message cache, plaintext, short-lived (legacy — prefer cacheEmail below)
  cacheFullMessage: (folder: string, uid: number, data: unknown) =>
    put('fullMessages', `${folder}:${uid}`, data),
  getFullMessage: (folder: string, uid: number) =>
    get<unknown>('fullMessages', `${folder}:${uid}`, 30 * 60 * 1000),

  // Added (TMAIL-87): encrypted email body cache with 30-day retention.
  // `receivedAt` is the original message Date in ms — drives retention pruning,
  // not the cache write time. Quota-aware: refuses to grow past MAX_CACHE_BYTES.
  async cacheEmail<T>(folder: string, uid: number, data: T, receivedAt?: number): Promise<void> {
    const currentBytes = await getEmailsCacheBytes();
    if (currentBytes >= MAX_CACHE_BYTES) {
      // Try to free space by retention pruning first
      await offlineCache.pruneOldEmails();
      const afterPrune = await getEmailsCacheBytes();
      if (afterPrune >= MAX_CACHE_BYTES) return;
    }
    // Best-effort browser quota check
    if (await isLowOnSpace()) {
      await offlineCache.pruneOldEmails();
    }
    const envelope = await encryptJson(data);
    await putEncrypted('emails', `${folder}:${uid}`, envelope, receivedAt);
  },

  async getEmail<T>(folder: string, uid: number): Promise<CachedEmail<T> | null> {
    const entry = await getEncryptedEntry('emails', `${folder}:${uid}`);
    if (!entry) return null;
    try {
      const data = await decryptJson<T>(entry.envelope);
      return { data, cachedAt: entry.cachedAt, receivedAt: entry.receivedAt };
    } catch {
      // Decryption failure usually means the session key was rotated (logout).
      // Treat as cache miss and let the caller re-fetch.
      return null;
    }
  },

  // Added (TMAIL-87): encrypted local drafts. localId is a client-generated UUID
  // so an unsynced draft survives across reloads before it gets a server-side id.
  async cacheDraft<T>(localId: string, data: T): Promise<void> {
    const envelope = await encryptJson(data);
    await putEncrypted('drafts', localId, envelope);
  },

  async getDraft<T>(localId: string): Promise<CachedDraft<T> | null> {
    const entry = await getEncryptedEntry('drafts', localId);
    if (!entry) return null;
    try {
      const data = await decryptJson<T>(entry.envelope);
      return { data, cachedAt: entry.cachedAt };
    } catch {
      return null;
    }
  },

  async listDrafts<T>(): Promise<Array<{ localId: string } & CachedDraft<T>>> {
    const db = await openDB();
    // NOTE: We must collect raw entries synchronously inside the transaction
    // (IDB transactions go inactive across microtasks), then decrypt outside.
    const rawEntries = await new Promise<EncryptedCacheEntry[]>((resolve, reject) => {
      const tx = db.transaction('drafts', 'readonly');
      const req = tx.objectStore('drafts').openCursor();
      const collected: EncryptedCacheEntry[] = [];
      req.onsuccess = () => {
        const cursor = req.result;
        if (!cursor) return;
        collected.push(cursor.value as EncryptedCacheEntry);
        cursor.continue();
      };
      tx.oncomplete = () => resolve(collected);
      tx.onerror = () => reject(tx.error);
    });

    const out: Array<{ localId: string } & CachedDraft<T>> = [];
    for (const entry of rawEntries) {
      try {
        const data = await decryptJson<T>(entry.envelope);
        out.push({ localId: entry.key, data, cachedAt: entry.cachedAt });
      } catch {
        // skip undecryptable entries (key rotated)
      }
    }
    return out;
  },

  async removeDraft(localId: string): Promise<void> {
    const db = await openDB();
    return new Promise((resolve, reject) => {
      const tx = db.transaction('drafts', 'readwrite');
      tx.objectStore('drafts').delete(localId);
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
  },

  // Added (TMAIL-87): drop emails older than the retention window. Uses receivedAt
  // when present (original message date), falling back to cachedAt.
  async pruneOldEmails(maxAgeDays: number = EMAIL_RETENTION_DAYS): Promise<number> {
    const cutoff = Date.now() - maxAgeDays * 24 * 60 * 60 * 1000;
    return pruneStore('emails', (entry) => {
      const ts = entry.receivedAt ?? entry.cachedAt;
      return ts < cutoff;
    });
  },

  // Added (TMAIL-87): proactive quota guard — caller can run periodically.
  // Returns count of evicted entries.
  async pruneIfLowOnSpace(): Promise<number> {
    if (!(await isLowOnSpace())) return 0;
    return offlineCache.pruneOldEmails();
  },

  // Added (TMAIL-87): bytes currently used by the encrypted emails store.
  getEmailsCacheBytes,

  // Clear all caches
  clearAll: async () => {
    const db = await openDB();
    const stores = ['folders', 'messages', 'fullMessages', 'emails', 'drafts'];
    const present = stores.filter((s) => db.objectStoreNames.contains(s));
    const tx = db.transaction(present, 'readwrite');
    for (const s of present) tx.objectStore(s).clear();
    return new Promise<void>((resolve, reject) => {
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
  },
};
