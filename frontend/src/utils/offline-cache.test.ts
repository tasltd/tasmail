import { describe, it, expect, beforeEach } from 'vitest';
import 'fake-indexeddb/auto';
import { offlineCache } from './offline-cache';
import { clearSessionKey } from './offline-encryption';

// NOTE: fake-indexeddb/auto polyfills global indexedDB for jsdom

describe('offlineCache', () => {
  beforeEach(async () => {
    await offlineCache.clearAll();
    await clearSessionKey();
  });

  describe('folders', () => {
    it('caches and retrieves folders within TTL', async () => {
      const folders = [{ name: 'INBOX', delimiter: '/', messages: 10, unseen: 2 }];
      await offlineCache.cacheFolders(folders);
      const result = await offlineCache.getFolders();
      expect(result).toEqual(folders);
    });

    it('returns null when no cached folders', async () => {
      const result = await offlineCache.getFolders();
      expect(result).toBeNull();
    });
  });

  describe('messages', () => {
    it('caches and retrieves message list per folder/page', async () => {
      const data = { messages: [{ uid: 1, subject: 'Test' }], total: 1, page: 0, page_size: 50 };
      await offlineCache.cacheMessages('INBOX', 0, data);
      const result = await offlineCache.getMessages('INBOX', 0);
      expect(result).toEqual(data);
    });

    it('returns null for uncached folder/page combination', async () => {
      const data = { messages: [], total: 0, page: 0, page_size: 50 };
      await offlineCache.cacheMessages('INBOX', 0, data);
      const result = await offlineCache.getMessages('Sent', 0);
      expect(result).toBeNull();
    });

    it('separates pages for same folder', async () => {
      const page0 = { messages: [{ uid: 1 }], total: 100, page: 0, page_size: 50 };
      const page1 = { messages: [{ uid: 51 }], total: 100, page: 1, page_size: 50 };
      await offlineCache.cacheMessages('INBOX', 0, page0);
      await offlineCache.cacheMessages('INBOX', 1, page1);
      expect(await offlineCache.getMessages('INBOX', 0)).toEqual(page0);
      expect(await offlineCache.getMessages('INBOX', 1)).toEqual(page1);
    });
  });

  describe('fullMessages', () => {
    it('caches and retrieves a full message', async () => {
      const msg = { uid: 42, subject: 'Hello', from: 'a@b.c', html_body: '<p>Hi</p>' };
      await offlineCache.cacheFullMessage('INBOX', 42, msg);
      const result = await offlineCache.getFullMessage('INBOX', 42);
      expect(result).toEqual(msg);
    });

    it('returns null for unknown message', async () => {
      const result = await offlineCache.getFullMessage('INBOX', 999);
      expect(result).toBeNull();
    });
  });

  describe('clearAll', () => {
    it('clears all object stores', async () => {
      await offlineCache.cacheFolders([{ name: 'INBOX' }]);
      await offlineCache.cacheMessages('INBOX', 0, { messages: [] });
      await offlineCache.cacheFullMessage('INBOX', 1, { uid: 1 });

      await offlineCache.clearAll();

      expect(await offlineCache.getFolders()).toBeNull();
      expect(await offlineCache.getMessages('INBOX', 0)).toBeNull();
      expect(await offlineCache.getFullMessage('INBOX', 1)).toBeNull();
    });
  });

  // ---- TMAIL-87: encrypted emails store ----
  describe('encrypted emails store', () => {
    it('round-trips an encrypted email body', async () => {
      const msg = {
        uid: 42,
        subject: 'Secret memo',
        from: 'boss@example.com',
        html_body: '<p>do not leak</p>',
      };
      await offlineCache.cacheEmail('INBOX', 42, msg);
      const result = await offlineCache.getEmail<typeof msg>('INBOX', 42);
      expect(result).not.toBeNull();
      expect(result!.data).toEqual(msg);
      expect(typeof result!.cachedAt).toBe('number');
    });

    it('stores ciphertext, not plaintext, in IndexedDB', async () => {
      const msg = { uid: 1, subject: 'plaintext-marker-XYZ' };
      await offlineCache.cacheEmail('INBOX', 1, msg);

      // Reach into the raw store and confirm the marker is not present in clear
      const db = await new Promise<IDBDatabase>((resolve, reject) => {
        const req = indexedDB.open('tasmail-cache');
        req.onsuccess = () => resolve(req.result);
        req.onerror = () => reject(req.error);
      });
      const raw = await new Promise<unknown>((resolve, reject) => {
        const tx = db.transaction('emails', 'readonly');
        const r = tx.objectStore('emails').get('INBOX:1');
        r.onsuccess = () => resolve(r.result);
        r.onerror = () => reject(r.error);
      });
      expect(JSON.stringify(raw)).not.toContain('plaintext-marker-XYZ');
    });

    it('returns null for an unknown email', async () => {
      expect(await offlineCache.getEmail('INBOX', 9999)).toBeNull();
    });

    it('preserves receivedAt metadata for retention', async () => {
      const tenDaysAgo = Date.now() - 10 * 24 * 60 * 60 * 1000;
      await offlineCache.cacheEmail('INBOX', 7, { uid: 7 }, tenDaysAgo);
      const result = await offlineCache.getEmail('INBOX', 7);
      expect(result!.receivedAt).toBe(tenDaysAgo);
    });
  });

  // ---- TMAIL-87: drafts store ----
  describe('encrypted drafts store', () => {
    it('round-trips a draft', async () => {
      const draft = { to: ['x@y.z'], subject: 'WIP', html_body: '<p>typing…</p>' };
      await offlineCache.cacheDraft('local-1', draft);
      const result = await offlineCache.getDraft<typeof draft>('local-1');
      expect(result!.data).toEqual(draft);
    });

    it('lists all drafts', async () => {
      await offlineCache.cacheDraft('a', { subject: 'A' });
      await offlineCache.cacheDraft('b', { subject: 'B' });
      const list = await offlineCache.listDrafts<{ subject: string }>();
      expect(list).toHaveLength(2);
      expect(list.map((d) => d.localId).sort()).toEqual(['a', 'b']);
    });

    it('removes a draft by id', async () => {
      await offlineCache.cacheDraft('to-remove', { subject: 'bye' });
      await offlineCache.removeDraft('to-remove');
      expect(await offlineCache.getDraft('to-remove')).toBeNull();
    });

    it('returns null for unknown draft id', async () => {
      expect(await offlineCache.getDraft('nope')).toBeNull();
    });
  });

  // ---- TMAIL-87: retention pruning ----
  describe('pruneOldEmails', () => {
    it('removes emails older than the retention window (by receivedAt)', async () => {
      const now = Date.now();
      const old = now - 40 * 24 * 60 * 60 * 1000;     // 40 days ago — should prune
      const fresh = now - 5 * 24 * 60 * 60 * 1000;    // 5 days ago — keep
      await offlineCache.cacheEmail('INBOX', 1, { uid: 1 }, old);
      await offlineCache.cacheEmail('INBOX', 2, { uid: 2 }, fresh);

      const deleted = await offlineCache.pruneOldEmails(30);

      expect(deleted).toBe(1);
      expect(await offlineCache.getEmail('INBOX', 1)).toBeNull();
      expect(await offlineCache.getEmail('INBOX', 2)).not.toBeNull();
    });

    it('falls back to cachedAt when receivedAt is missing', async () => {
      await offlineCache.cacheEmail('INBOX', 1, { uid: 1 });
      const deleted = await offlineCache.pruneOldEmails(30);
      expect(deleted).toBe(0);
    });

    it('respects custom retention windows', async () => {
      const oneDayAgo = Date.now() - 24 * 60 * 60 * 1000;
      await offlineCache.cacheEmail('INBOX', 1, { uid: 1 }, oneDayAgo);
      expect(await offlineCache.pruneOldEmails(7)).toBe(0);   // keep
      expect(await offlineCache.pruneOldEmails(0.5)).toBe(1); // prune (< 12 hours)
    });
  });

  // ---- TMAIL-87: post-logout key clearing ----
  describe('post-key-rotation behavior', () => {
    it('treats prior encrypted entries as cache miss after clearSessionKey', async () => {
      await offlineCache.cacheEmail('INBOX', 1, { uid: 1, secret: 'shh' });
      await clearSessionKey();
      // New key gets generated on next access; old ciphertext won't decrypt → null
      expect(await offlineCache.getEmail('INBOX', 1)).toBeNull();
    });
  });
});
