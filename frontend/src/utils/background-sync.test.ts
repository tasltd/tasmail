import { describe, it, expect, beforeEach, vi } from 'vitest';
import 'fake-indexeddb/auto';
import { backgroundSync } from './background-sync';

describe('backgroundSync', () => {
  beforeEach(async () => {
    await backgroundSync.clearAll();
  });

  describe('enqueue', () => {
    it('adds an action and returns an id', async () => {
      const id = await backgroundSync.enqueue('send', { to: ['a@b.c'], subject: 'Hi' });
      expect(typeof id).toBe('number');
      expect(id).toBeGreaterThan(0);
    });

    it('enqueues multiple actions with unique ids', async () => {
      const id1 = await backgroundSync.enqueue('move', { folder: 'INBOX', uid: 1, toFolder: 'Trash' });
      const id2 = await backgroundSync.enqueue('delete', { folder: 'Trash', uid: 2 });
      expect(id1).not.toBe(id2);
    });
  });

  describe('getPending', () => {
    it('returns empty array when no actions queued', async () => {
      const actions = await backgroundSync.getPending();
      expect(actions).toEqual([]);
    });

    it('returns all queued actions', async () => {
      await backgroundSync.enqueue('flag', { folder: 'INBOX', uid: 1, flag: '\\Seen', add: true });
      await backgroundSync.enqueue('delete', { folder: 'INBOX', uid: 2 });
      const actions = await backgroundSync.getPending();
      expect(actions).toHaveLength(2);
      expect(actions[0].type).toBe('flag');
      expect(actions[1].type).toBe('delete');
    });

    it('stores payload and metadata correctly', async () => {
      const before = Date.now();
      await backgroundSync.enqueue('send', { to: ['x@y.z'], subject: 'Test' });
      const actions = await backgroundSync.getPending();
      expect(actions[0].payload).toEqual({ to: ['x@y.z'], subject: 'Test' });
      expect(actions[0].retries).toBe(0);
      expect(actions[0].createdAt).toBeGreaterThanOrEqual(before);
    });
  });

  describe('remove', () => {
    it('removes a specific action by id', async () => {
      const id1 = await backgroundSync.enqueue('send', { to: ['a@b.c'], subject: 'A' });
      await backgroundSync.enqueue('send', { to: ['d@e.f'], subject: 'B' });
      await backgroundSync.remove(id1);
      const remaining = await backgroundSync.getPending();
      expect(remaining).toHaveLength(1);
      expect(remaining[0].payload.subject).toBe('B');
    });
  });

  describe('clearAll', () => {
    it('removes all pending actions', async () => {
      await backgroundSync.enqueue('send', { to: ['a@b.c'], subject: '1' });
      await backgroundSync.enqueue('delete', { folder: 'INBOX', uid: 1 });
      await backgroundSync.clearAll();
      const actions = await backgroundSync.getPending();
      expect(actions).toEqual([]);
    });
  });

  describe('getPendingCount', () => {
    it('returns 0 when queue is empty', async () => {
      expect(await backgroundSync.getPendingCount()).toBe(0);
    });

    it('returns exact queue size', async () => {
      await backgroundSync.enqueue('send', { to: ['a@b.c'], subject: '1' });
      await backgroundSync.enqueue('move', { folder: 'INBOX', uid: 1, toFolder: 'Trash' });
      await backgroundSync.enqueue('delete', { folder: 'INBOX', uid: 2 });
      expect(await backgroundSync.getPendingCount()).toBe(3);
    });
  });

  describe('subscribe (TMAIL-88)', () => {
    it('fires listener on enqueue', async () => {
      const listener = vi.fn();
      const unsubscribe = backgroundSync.subscribe(listener);
      await backgroundSync.enqueue('delete', { folder: 'INBOX', uid: 1 });
      expect(listener).toHaveBeenCalled();
      unsubscribe();
    });

    it('fires listener on clearAll', async () => {
      await backgroundSync.enqueue('delete', { folder: 'INBOX', uid: 1 });
      const listener = vi.fn();
      const unsubscribe = backgroundSync.subscribe(listener);
      await backgroundSync.clearAll();
      expect(listener).toHaveBeenCalled();
      unsubscribe();
    });

    it('does not fire after unsubscribe', async () => {
      const listener = vi.fn();
      const unsubscribe = backgroundSync.subscribe(listener);
      unsubscribe();
      await backgroundSync.enqueue('delete', { folder: 'INBOX', uid: 1 });
      expect(listener).not.toHaveBeenCalled();
    });

    it('isolates a faulty listener from the rest', async () => {
      const good = vi.fn();
      const bad = vi.fn(() => { throw new Error('boom'); });
      // Silence the expected console.error noise
      const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
      const u1 = backgroundSync.subscribe(bad);
      const u2 = backgroundSync.subscribe(good);
      await backgroundSync.enqueue('delete', { folder: 'INBOX', uid: 1 });
      expect(good).toHaveBeenCalled();
      expect(bad).toHaveBeenCalled();
      errSpy.mockRestore();
      u1(); u2();
    });
  });

  describe('LWW order (TMAIL-88)', () => {
    it('getPending returns actions sorted by createdAt ascending', async () => {
      // Force three distinct createdAt by mocking Date.now between enqueues.
      let t = 1_000_000;
      const dateSpy = vi.spyOn(Date, 'now').mockImplementation(() => t);

      t = 3000; await backgroundSync.enqueue('send', { to: ['c'], subject: '3rd' });
      t = 1000; await backgroundSync.enqueue('send', { to: ['a'], subject: '1st' });
      t = 2000; await backgroundSync.enqueue('send', { to: ['b'], subject: '2nd' });

      const actions = await backgroundSync.getPending();
      expect(actions.map((a) => a.payload.subject)).toEqual(['1st', '2nd', '3rd']);

      dateSpy.mockRestore();
    });
  });

  describe('processPending', () => {
    it('returns zero counts when queue is empty', async () => {
      const result = await backgroundSync.processPending();
      expect(result).toEqual({ processed: 0, failed: 0 });
    });

    it('removes actions that exceed max retries (3)', async () => {
      // Manually create an action with 3 retries to simulate repeated failures
      await backgroundSync.enqueue('send', { to: ['a@b.c'], subject: 'fail' });
      const actions = await backgroundSync.getPending();
      // Patch retries to 3 via direct IDB manipulation
      const db = await openSyncDB();
      const tx = db.transaction('pending-actions', 'readwrite');
      const store = tx.objectStore('pending-actions');
      const action = actions[0];
      action.retries = 3;
      store.put(action);
      await new Promise<void>((r) => { tx.oncomplete = () => r(); });

      const result = await backgroundSync.processPending();
      expect(result.failed).toBe(1);
      expect(result.processed).toBe(0);
      // Action should be removed from queue
      const remaining = await backgroundSync.getPending();
      expect(remaining).toHaveLength(0);
    });
  });
});

// Helper: open the sync DB directly for test manipulation
function openSyncDB(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open('tasmail-sync', 1);
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}
