// Added (TMAIL-89): unit tests for the offline-drafts module.
import { describe, it, expect, beforeEach } from 'vitest';
import 'fake-indexeddb/auto';
import { offlineCache } from './offline-cache';
import { clearSessionKey } from './offline-encryption';
import {
  createEmptyDraft,
  applyEdits,
  isDirty,
  markSynced,
  markConflict,
  markError,
  saveDraftLocal,
  loadDraft,
  deleteDraftLocal,
  listLocalDrafts,
  addAttachment,
  removeAttachment,
  loadAttachmentBlob,
  syncOne,
  syncAllDirty,
  statusBadge,
  newLocalId,
  MAX_ATTACHMENT_BYTES,
  MAX_ATTACHMENTS_PER_DRAFT,
  AttachmentQuotaError,
  _resetAttachmentsForTests,
  type OfflineDraft,
  type SyncContext,
} from './offline-drafts';

// Reset all IndexedDB state between tests so caches don't leak.
beforeEach(async () => {
  await offlineCache.clearAll();
  await clearSessionKey();
  await _resetAttachmentsForTests();
});

describe('pure helpers', () => {
  it('newLocalId returns a non-empty unique string', () => {
    const a = newLocalId();
    const b = newLocalId();
    expect(a).toBeTruthy();
    expect(b).toBeTruthy();
    expect(a).not.toBe(b);
  });

  it('createEmptyDraft initialises a clean local draft', () => {
    const draft = createEmptyDraft('id-1');
    expect(draft.localId).toBe('id-1');
    expect(draft.to).toBe('');
    expect(draft.subject).toBe('');
    expect(draft.attachments).toEqual([]);
    expect(draft.status).toBe('local');
    expect(draft.syncedVersion).toBe(0);
    expect(draft.lastEditedAt).toBeGreaterThan(0);
  });

  it('applyEdits patches fields, bumps lastEditedAt, and resets status to local', async () => {
    const original = createEmptyDraft('id-1');
    const synced = markSynced(original);
    expect(synced.status).toBe('synced');

    // Wait a tick so Date.now() advances.
    await new Promise((r) => setTimeout(r, 2));
    const edited = applyEdits(synced, { subject: 'Hello' });
    expect(edited.subject).toBe('Hello');
    expect(edited.status).toBe('local');
    expect(edited.lastEditedAt).toBeGreaterThan(synced.lastEditedAt);
  });

  it('applyEdits preserves conflict status (does not overwrite it)', () => {
    const draft = markConflict(createEmptyDraft('id-1'), 12345);
    const edited = applyEdits(draft, { subject: 'still conflicted' });
    expect(edited.status).toBe('conflict');
  });

  it('applyEdits is a no-op when no field actually changes', () => {
    const draft = markSynced(applyEdits(createEmptyDraft('id-1'), { subject: 'Hi' }));
    // Re-apply the same subject — status should NOT flip back to local.
    const reapplied = applyEdits(draft, { subject: 'Hi' });
    expect(reapplied).toBe(draft);
    expect(reapplied.status).toBe('synced');
  });

  it('isDirty is true for new drafts and false right after sync', () => {
    const draft = createEmptyDraft('id-1');
    expect(isDirty(draft)).toBe(true);
    const synced = markSynced(draft);
    expect(isDirty(synced)).toBe(false);
  });

  it('isDirty flags conflicts and errors regardless of versions', () => {
    const draft = markSynced(createEmptyDraft('id-1'));
    expect(isDirty(draft)).toBe(false);
    expect(isDirty(markConflict(draft, 999))).toBe(true);
    expect(isDirty(markError(draft, 'oops'))).toBe(true);
  });

  it('statusBadge returns correct label and tone per status', () => {
    expect(statusBadge('local').label).toBe('Saved locally');
    expect(statusBadge('syncing').label).toBe('Syncing…');
    expect(statusBadge('synced').label).toBe('Synced to server');
    expect(statusBadge('synced').tone).toBe('good');
    expect(statusBadge('conflict').tone).toBe('warn');
    expect(statusBadge('error').tone).toBe('error');
  });
});

describe('persistence', () => {
  it('round-trips a draft through the encrypted store', async () => {
    const draft = applyEdits(createEmptyDraft('id-1'), {
      to: 'alice@example.com',
      subject: 'Hello',
      htmlBody: '<p>hi</p>',
      textBody: 'hi',
    });
    await saveDraftLocal(draft);
    const loaded = await loadDraft('id-1');
    expect(loaded).not.toBeNull();
    expect(loaded?.to).toBe('alice@example.com');
    expect(loaded?.subject).toBe('Hello');
  });

  it('loadDraft returns null for unknown id', async () => {
    expect(await loadDraft('does-not-exist')).toBeNull();
  });

  it('listLocalDrafts sorts by lastEditedAt descending', async () => {
    const a = applyEdits(createEmptyDraft('a'), { subject: 'A' });
    await saveDraftLocal(a);
    await new Promise((r) => setTimeout(r, 5));
    const b = applyEdits(createEmptyDraft('b'), { subject: 'B' });
    await saveDraftLocal(b);

    const list = await listLocalDrafts();
    expect(list).toHaveLength(2);
    expect(list[0].localId).toBe('b');
    expect(list[1].localId).toBe('a');
  });

  it('deleteDraftLocal removes the draft and its attachments', async () => {
    const draft = createEmptyDraft('id-1');
    await saveDraftLocal(draft);
    const { draft: withAttach } = await addAttachment(draft, {
      name: 'x.txt',
      type: 'text/plain',
      size: 5,
      blob: new Blob(['hello']),
    });
    await saveDraftLocal(withAttach);

    await deleteDraftLocal('id-1');
    expect(await loadDraft('id-1')).toBeNull();
    const blob = await loadAttachmentBlob('id-1', withAttach.attachments[0].id);
    expect(blob).toBeNull();
  });
});

describe('attachments', () => {
  it('adds an attachment and stores its Blob in IndexedDB', async () => {
    const draft = createEmptyDraft('id-1');
    const blob = new Blob(['hello world'], { type: 'text/plain' });
    const { draft: updated, attachment } = await addAttachment(draft, {
      name: 'greeting.txt',
      type: 'text/plain',
      size: blob.size,
      blob,
    });

    expect(updated.attachments).toHaveLength(1);
    expect(attachment.filename).toBe('greeting.txt');
    expect(attachment.size).toBe(blob.size);

    const loaded = await loadAttachmentBlob('id-1', attachment.id);
    expect(loaded).not.toBeNull();
    // NOTE: fake-indexeddb's structured-clone strips the Blob prototype, so
    // we assert on the metadata kept on the draft rather than the live Blob
    // instance's `.size` getter. In real browsers the size getter round-trips.
    expect(attachment.size).toBe(blob.size);
  });

  it('rejects an attachment over the per-file size cap', async () => {
    const draft = createEmptyDraft('id-1');
    // We can't actually allocate 25 MB in the test, but we can lie about size
    // — the quota check uses `file.size` directly.
    const fakeBig = new Blob(['x']);
    await expect(
      addAttachment(draft, {
        name: 'huge.bin',
        type: 'application/octet-stream',
        size: MAX_ATTACHMENT_BYTES + 1,
        blob: fakeBig,
      }),
    ).rejects.toBeInstanceOf(AttachmentQuotaError);
  });

  it('rejects attachments when the per-draft count cap is hit', async () => {
    let draft = createEmptyDraft('id-1');
    for (let i = 0; i < MAX_ATTACHMENTS_PER_DRAFT; i++) {
      const { draft: next } = await addAttachment(draft, {
        name: `${i}.txt`,
        type: 'text/plain',
        size: 1,
        blob: new Blob(['x']),
      });
      draft = next;
    }
    await expect(
      addAttachment(draft, { name: 'overflow.txt', type: 'text/plain', size: 1, blob: new Blob(['x']) }),
    ).rejects.toBeInstanceOf(AttachmentQuotaError);
  });

  it('removeAttachment drops the meta entry and the blob', async () => {
    const draft = createEmptyDraft('id-1');
    const { draft: withAttach, attachment } = await addAttachment(draft, {
      name: 'a.txt',
      type: 'text/plain',
      size: 1,
      blob: new Blob(['x']),
    });
    const after = await removeAttachment(withAttach, attachment.id);
    expect(after.attachments).toHaveLength(0);
    expect(await loadAttachmentBlob('id-1', attachment.id)).toBeNull();
  });
});

describe('syncOne', () => {
  function makeCtx(overrides: Partial<SyncContext> = {}): SyncContext {
    return {
      postDraft: async () => ({ status: 'ok' }),
      ...overrides,
    };
  }

  it('flips a dirty draft to synced on a successful post', async () => {
    const draft = applyEdits(createEmptyDraft('id-1'), { subject: 'Hi' });
    await saveDraftLocal(draft);

    const result = await syncOne(draft, makeCtx());
    expect(result.status).toBe('synced');
    expect(result.syncedVersion).toBe(draft.lastEditedAt);

    const reloaded = await loadDraft('id-1');
    expect(reloaded?.status).toBe('synced');
  });

  it('is a no-op when the draft has nothing new to sync', async () => {
    const baseline = markSynced(createEmptyDraft('id-1'));
    await saveDraftLocal(baseline);
    let calls = 0;
    const ctx = makeCtx({
      postDraft: async () => {
        calls += 1;
        return { status: 'ok' };
      },
    });
    const result = await syncOne(baseline, ctx);
    expect(calls).toBe(0);
    expect(result).toEqual(baseline);
  });

  it('marks the draft as conflict when the server reports one', async () => {
    const draft = applyEdits(createEmptyDraft('id-1'), { subject: 'Hi' });
    await saveDraftLocal(draft);

    const result = await syncOne(draft, makeCtx({
      postDraft: async () => ({ status: 'conflict', serverVersion: 9999 }),
    }));
    expect(result.status).toBe('conflict');
    expect(result.serverConflictVersion).toBe(9999);

    const reloaded = await loadDraft('id-1');
    expect(reloaded?.status).toBe('conflict');
  });

  it('marks the draft as error on a network failure and keeps it dirty', async () => {
    const draft = applyEdits(createEmptyDraft('id-1'), { subject: 'Hi' });
    await saveDraftLocal(draft);

    const result = await syncOne(draft, makeCtx({
      postDraft: async () => { throw new Error('network down'); },
    }));
    expect(result.status).toBe('error');
    expect(result.errorMessage).toBe('network down');
    expect(isDirty(result)).toBe(true);
  });

  it('writes a "syncing" snapshot to IndexedDB before resolving', async () => {
    const draft = applyEdits(createEmptyDraft('id-1'), { subject: 'Hi' });
    await saveDraftLocal(draft);

    let snapshotDuringFlight: OfflineDraft | null = null;
    const ctx = makeCtx({
      postDraft: async () => {
        snapshotDuringFlight = await loadDraft('id-1');
        return { status: 'ok' };
      },
    });
    await syncOne(draft, ctx);
    expect(snapshotDuringFlight).not.toBeNull();
    expect(snapshotDuringFlight!.status).toBe('syncing');
  });
});

describe('syncAllDirty', () => {
  it('syncs only dirty drafts and returns counts by outcome', async () => {
    const a = applyEdits(createEmptyDraft('a'), { subject: 'A' });
    const b = applyEdits(createEmptyDraft('b'), { subject: 'B' });
    const c = markSynced(createEmptyDraft('c')); // already clean
    await saveDraftLocal(a);
    await saveDraftLocal(b);
    await saveDraftLocal(c);

    let calls = 0;
    const result = await syncAllDirty({
      postDraft: async (d) => {
        calls += 1;
        if (d.localId === 'b') return { status: 'conflict', serverVersion: 100 };
        return { status: 'ok' };
      },
    });
    expect(calls).toBe(2);
    expect(result.synced).toBe(1);
    expect(result.conflicts).toBe(1);
    expect(result.errors).toBe(0);
  });

  it('counts errors when the network fails on some drafts', async () => {
    const a = applyEdits(createEmptyDraft('a'), { subject: 'A' });
    await saveDraftLocal(a);

    const result = await syncAllDirty({
      postDraft: async () => { throw new Error('boom'); },
    });
    expect(result.errors).toBe(1);
    expect(result.synced).toBe(0);
  });
});
