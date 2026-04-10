/**
 * IndexedDB-based offline email cache for PWA support.
 * Caches folder lists, message envelopes, and full messages.
 */

const DB_NAME = 'tasmail-cache';
const DB_VERSION = 1;

interface CacheEntry<T> {
  key: string;
  data: T;
  cachedAt: number;
}

function openDB(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);

    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains('folders')) {
        db.createObjectStore('folders', { keyPath: 'key' });
      }
      if (!db.objectStoreNames.contains('messages')) {
        db.createObjectStore('messages', { keyPath: 'key' });
      }
      if (!db.objectStoreNames.contains('fullMessages')) {
        db.createObjectStore('fullMessages', { keyPath: 'key' });
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

export const offlineCache = {
  // Folders cache (5 min TTL)
  cacheFolders: (folders: unknown) => put('folders', 'all', folders),
  getFolders: () => get<unknown>('folders', 'all', 5 * 60 * 1000),

  // Message list cache per folder (2 min TTL)
  cacheMessages: (folder: string, page: number, data: unknown) =>
    put('messages', `${folder}:${page}`, data),
  getMessages: (folder: string, page: number) =>
    get<unknown>('messages', `${folder}:${page}`, 2 * 60 * 1000),

  // Full message cache (30 min TTL)
  cacheFullMessage: (folder: string, uid: number, data: unknown) =>
    put('fullMessages', `${folder}:${uid}`, data),
  getFullMessage: (folder: string, uid: number) =>
    get<unknown>('fullMessages', `${folder}:${uid}`, 30 * 60 * 1000),

  // Clear all caches
  clearAll: async () => {
    const db = await openDB();
    const tx = db.transaction(['folders', 'messages', 'fullMessages'], 'readwrite');
    tx.objectStore('folders').clear();
    tx.objectStore('messages').clear();
    tx.objectStore('fullMessages').clear();
  },
};
