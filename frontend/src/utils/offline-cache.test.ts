import { describe, it, expect, beforeEach } from 'vitest';
import 'fake-indexeddb/auto';
import { offlineCache } from './offline-cache';

// NOTE: fake-indexeddb/auto polyfills global indexedDB for jsdom

describe('offlineCache', () => {
  beforeEach(async () => {
    await offlineCache.clearAll();
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
});
