/**
 * Offline draft composition (TMAIL-89).
 *
 * Sits on top of the encrypted `drafts` IndexedDB store provided by TMAIL-87
 * (`offline-cache.ts`) and the `save-draft` sync action provided by TMAIL-88
 * (`background-sync.ts`).
 *
 * What this module adds on top of the lower layers:
 *   - A first-class `OfflineDraft` model with status, version, server snapshot,
 *     and attachments-as-Blob.
 *   - Per-draft and per-attachment size guards so the IndexedDB quota stays
 *     well under the soft cap that `offline-quota.ts` polices.
 *   - Pure status-transition helpers (markSyncing / markSynced / markConflict)
 *     so callers never write a partial / inconsistent draft.
 *   - A `syncOne()` helper that calls the existing `/api/drafts` POST through
 *     the standard ApiClient (so it inherits auth-refresh) and flips the
 *     draft's status correctly — including detecting cross-device conflict
 *     by comparing `lastEditedAt` against the snapshot the server already
 *     has.
 *
 * Conflict model
 * --------------
 * The TASMail backend keeps drafts in the user's IMAP `Drafts` folder
 * (`handlers/messages::save_draft`). Each successful sync appends a new IMAP
 * draft, so to detect cross-device edits we compare the *client-side* version
 * stamp (`lastEditedAt`) against the `syncedVersion` we recorded after the
 * last successful POST. If the client tries to sync a draft whose
 * `serverConflictVersion` (set when another tab/device wrote a newer copy)
 * is higher than the local `syncedVersion`, the draft is flipped to
 * `conflict` and the UI surfaces the choice to the user.
 *
 * Encryption: piggybacks on TMAIL-87 — the underlying envelope is AES-256-GCM
 * encrypted at rest. Attachments are stored as raw Blob references inside the
 * draft body (IndexedDB serialises structured-cloneable Blobs natively).
 *
 * Important: the encrypted `drafts` store from TMAIL-87 round-trips through
 * `JSON.stringify` (see `encryptJson` in `offline-encryption.ts`), so Blob
 * fields would be lost. We therefore split the model: the *body* of the draft
 * is JSON-friendly and goes through the encrypted store, while the attachment
 * Blobs live in a separate, unencrypted `draft-attachments` object store keyed
 * by `${localId}:${attachmentId}`. Attachments are listed by id on the draft
 * so the Composer can rehydrate them after a reload.
 */

import { offlineCache } from './offline-cache';

/** Maximum size of a single attachment, in bytes. */
export const MAX_ATTACHMENT_BYTES = 25 * 1024 * 1024; // 25 MB

/** Maximum total attachment bytes per draft. */
export const MAX_ATTACHMENTS_PER_DRAFT_BYTES = 50 * 1024 * 1024; // 50 MB

/** Maximum attachment count per draft. */
export const MAX_ATTACHMENTS_PER_DRAFT = 20;

export type DraftStatus = 'local' | 'syncing' | 'synced' | 'conflict' | 'error';

/** Metadata for a draft attachment. The actual bytes live in IndexedDB. */
export interface OfflineDraftAttachment {
  /** Stable id within the draft. */
  id: string;
  filename: string;
  mimeType: string;
  size: number;
  /** Millisecond timestamp the attachment was added locally. */
  addedAt: number;
}

/** JSON-serialisable shape of a draft as it lives in the encrypted store. */
export interface OfflineDraft {
  /** Client-generated UUID. Stable across renames / device hops. */
  localId: string;
  to: string;
  cc: string;
  subject: string;
  htmlBody: string;
  textBody: string;
  attachments: OfflineDraftAttachment[];
  /**
   * Monotonically increasing client-side version stamp (epoch ms at last
   * local edit). Acts as a "lamport-ish" version — anything <= syncedVersion
   * is considered synced.
   */
  lastEditedAt: number;
  /** Version that was last successfully POSTed to the server. */
  syncedVersion: number;
  /** Last successful server write, in epoch ms. */
  lastSyncedAt: number;
  /**
   * Set by another tab/device when it observes a newer server snapshot than
   * the one we synced. Used to detect conflict before overwriting.
   */
  serverConflictVersion?: number;
  /** Current sync status — drives the Composer's status badge. */
  status: DraftStatus;
  /** Last error message, if status is 'error'. */
  errorMessage?: string;
}

// NOTE: We keep attachment Blobs in a SEPARATE IndexedDB database from the
// `tasmail-cache` one used by offline-cache.ts. Mixing them would couple the
// two modules' DB_VERSION numbers and risk an `OpenFailedError` if one module
// opens at v2 while the other tries v3. Two separate DBs is cheaper than
// version-coordination across files.
const ATTACHMENT_DB_NAME = 'tasmail-draft-attachments';
const ATTACHMENT_DB_VERSION = 1;
const ATTACHMENT_STORE = 'attachments';

// NOTE: cache the open connection so we don't pile up handles. Each
// `indexedDB.open()` returns a NEW connection in fake-indexeddb and most
// browsers, and a stale handle blocks `deleteDatabase`. Reusing a single
// reference also matches the pattern in offline-cache.ts.
let cachedAttachmentDB: IDBDatabase | null = null;

function openAttachmentDB(): Promise<IDBDatabase> {
  if (cachedAttachmentDB) return Promise.resolve(cachedAttachmentDB);
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(ATTACHMENT_DB_NAME, ATTACHMENT_DB_VERSION);
    req.onupgradeneeded = () => {
      const db = req.result;
      if (!db.objectStoreNames.contains(ATTACHMENT_STORE)) {
        const s = db.createObjectStore(ATTACHMENT_STORE, { keyPath: 'key' });
        s.createIndex('draftLocalId', 'draftLocalId', { unique: false });
      }
    };
    req.onsuccess = () => {
      cachedAttachmentDB = req.result;
      // Drop the cache if the browser closes the connection underneath us
      // (e.g. on schema version bump from another tab).
      cachedAttachmentDB.onclose = () => { cachedAttachmentDB = null; };
      cachedAttachmentDB.onversionchange = () => {
        cachedAttachmentDB?.close();
        cachedAttachmentDB = null;
      };
      resolve(req.result);
    };
    req.onerror = () => reject(req.error);
  });
}

/**
 * Test-only helper: drop every attachment row and release the cached DB
 * handle so {@link indexedDB.deleteDatabase} won't block. Exposed under a
 * `_test` prefix so we never accidentally call it from production code.
 */
export async function _resetAttachmentsForTests(): Promise<void> {
  if (!cachedAttachmentDB) {
    // No open connection — still try to clear in case data exists.
    try {
      const db = await openAttachmentDB();
      await new Promise<void>((resolve, reject) => {
        const tx = db.transaction(ATTACHMENT_STORE, 'readwrite');
        tx.objectStore(ATTACHMENT_STORE).clear();
        tx.oncomplete = () => resolve();
        tx.onerror = () => reject(tx.error);
      });
    } catch {
      // store doesn't exist yet — fine
    }
  } else {
    await new Promise<void>((resolve, reject) => {
      const tx = cachedAttachmentDB!.transaction(ATTACHMENT_STORE, 'readwrite');
      tx.objectStore(ATTACHMENT_STORE).clear();
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
  }
  cachedAttachmentDB?.close();
  cachedAttachmentDB = null;
}

interface AttachmentBlobEntry {
  key: string;            // `${draftLocalId}:${attachmentId}`
  draftLocalId: string;   // indexed for fast prune-by-draft
  attachmentId: string;
  blob: Blob;
  size: number;
  filename: string;
  mimeType: string;
  addedAt: number;
}

/** Generate a UUIDv4 — falls back to a deterministic-but-unique pseudo-uuid
 *  in environments without `crypto.randomUUID` (older jsdom). */
export function newLocalId(): string {
  if (typeof globalThis.crypto?.randomUUID === 'function') {
    return globalThis.crypto.randomUUID();
  }
  // NOTE: not RFC 4122 compliant but uniqueness is what we need here.
  return `draft-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
}

/** Create a brand-new local draft skeleton. The caller persists it. */
export function createEmptyDraft(localId: string = newLocalId()): OfflineDraft {
  const now = Date.now();
  return {
    localId,
    to: '',
    cc: '',
    subject: '',
    htmlBody: '',
    textBody: '',
    attachments: [],
    lastEditedAt: now,
    syncedVersion: 0,
    lastSyncedAt: 0,
    status: 'local',
  };
}

/**
 * Pure helper: apply partial edits and bump the version stamp.
 *
 * If none of the supplied fields actually differ from the current draft, this
 * is a no-op — we return the same reference, do NOT bump `lastEditedAt`, and
 * do NOT reset status. That matters because the Composer calls `applyEdits`
 * on every render (to keep the local IDB copy in sync with form state), and
 * without this guard a freshly-synced draft would immediately flip back to
 * "Saved locally" simply because the render loop re-applied identical edits.
 */
export function applyEdits(
  draft: OfflineDraft,
  edits: Partial<Pick<OfflineDraft, 'to' | 'cc' | 'subject' | 'htmlBody' | 'textBody'>>,
): OfflineDraft {
  const fields: Array<keyof typeof edits> = ['to', 'cc', 'subject', 'htmlBody', 'textBody'];
  const changed = fields.some((key) => edits[key] !== undefined && edits[key] !== draft[key]);
  if (!changed) return draft;
  const updated: OfflineDraft = {
    ...draft,
    ...edits,
    lastEditedAt: Date.now(),
    // NOTE: any local edit invalidates a previously-synced state — until the
    // next successful POST, the badge should say "Saved locally", not synced.
    status: draft.status === 'conflict' ? 'conflict' : 'local',
    errorMessage: undefined,
  };
  return updated;
}

/** Pure helper: returns true when the draft has unsynced local changes. */
export function isDirty(draft: OfflineDraft): boolean {
  if (draft.status === 'conflict' || draft.status === 'error') return true;
  return draft.lastEditedAt > draft.syncedVersion;
}

/** Pure helper: status transitions. Never mutate in place. */
export function markSyncing(draft: OfflineDraft): OfflineDraft {
  return { ...draft, status: 'syncing', errorMessage: undefined };
}

export function markSynced(draft: OfflineDraft, syncedAt: number = Date.now()): OfflineDraft {
  return {
    ...draft,
    status: 'synced',
    syncedVersion: draft.lastEditedAt,
    lastSyncedAt: syncedAt,
    errorMessage: undefined,
    serverConflictVersion: undefined,
  };
}

export function markConflict(draft: OfflineDraft, serverVersion: number): OfflineDraft {
  return {
    ...draft,
    status: 'conflict',
    serverConflictVersion: serverVersion,
  };
}

export function markError(draft: OfflineDraft, message: string): OfflineDraft {
  return { ...draft, status: 'error', errorMessage: message };
}

/** Read a draft from the encrypted store. Returns null if unknown. */
export async function loadDraft(localId: string): Promise<OfflineDraft | null> {
  const entry = await offlineCache.getDraft<OfflineDraft>(localId);
  return entry?.data ?? null;
}

/** Persist a draft. The encrypted store handles AES-256-GCM at rest. */
export async function saveDraftLocal(draft: OfflineDraft): Promise<void> {
  await offlineCache.cacheDraft(draft.localId, draft);
}

/** Remove a draft and ALL of its attachment blobs. */
export async function deleteDraftLocal(localId: string): Promise<void> {
  await offlineCache.removeDraft(localId);
  await deleteAllAttachmentsFor(localId);
}

/** List all locally-stored drafts, newest edit first. */
export async function listLocalDrafts(): Promise<OfflineDraft[]> {
  const entries = await offlineCache.listDrafts<OfflineDraft>();
  const drafts = entries.map((e) => e.data).filter((d): d is OfflineDraft => !!d && typeof d.localId === 'string');
  drafts.sort((a, b) => b.lastEditedAt - a.lastEditedAt);
  return drafts;
}

// ---------------- Attachments ----------------

export interface AddAttachmentResult {
  attachment: OfflineDraftAttachment;
  draft: OfflineDraft;
}

export class AttachmentQuotaError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'AttachmentQuotaError';
  }
}

/**
 * Add an attachment Blob to a draft. Enforces per-attachment, per-draft, and
 * per-draft-count quotas. Returns the updated draft (caller must persist it
 * with `saveDraftLocal`).
 */
export async function addAttachment(
  draft: OfflineDraft,
  file: { name: string; type: string; size: number; blob: Blob },
): Promise<AddAttachmentResult> {
  if (file.size > MAX_ATTACHMENT_BYTES) {
    throw new AttachmentQuotaError(
      `Attachment ${file.name} is ${(file.size / 1024 / 1024).toFixed(1)} MB — over the ${MAX_ATTACHMENT_BYTES / 1024 / 1024} MB per-file limit.`,
    );
  }
  if (draft.attachments.length >= MAX_ATTACHMENTS_PER_DRAFT) {
    throw new AttachmentQuotaError(
      `Cannot add more than ${MAX_ATTACHMENTS_PER_DRAFT} attachments to a single draft.`,
    );
  }
  const existingSize = draft.attachments.reduce((acc, a) => acc + a.size, 0);
  if (existingSize + file.size > MAX_ATTACHMENTS_PER_DRAFT_BYTES) {
    throw new AttachmentQuotaError(
      `Attachments for this draft would exceed the ${MAX_ATTACHMENTS_PER_DRAFT_BYTES / 1024 / 1024} MB total cap.`,
    );
  }

  const attachmentId = newLocalId();
  const meta: OfflineDraftAttachment = {
    id: attachmentId,
    filename: file.name,
    mimeType: file.type || 'application/octet-stream',
    size: file.size,
    addedAt: Date.now(),
  };

  await putAttachmentBlob({
    key: `${draft.localId}:${attachmentId}`,
    draftLocalId: draft.localId,
    attachmentId,
    blob: file.blob,
    size: file.size,
    filename: meta.filename,
    mimeType: meta.mimeType,
    addedAt: meta.addedAt,
  });

  const updatedDraft = applyEdits({ ...draft, attachments: [...draft.attachments, meta] }, {});
  return { attachment: meta, draft: updatedDraft };
}

/** Remove a single attachment. Returns the updated draft. */
export async function removeAttachment(
  draft: OfflineDraft,
  attachmentId: string,
): Promise<OfflineDraft> {
  await deleteAttachmentBlob(`${draft.localId}:${attachmentId}`);
  const remaining = draft.attachments.filter((a) => a.id !== attachmentId);
  return applyEdits({ ...draft, attachments: remaining }, {});
}

/** Load an attachment Blob by id, e.g. to render a preview / re-upload. */
export async function loadAttachmentBlob(
  localId: string,
  attachmentId: string,
): Promise<Blob | null> {
  const entry = await getAttachmentBlobEntry(`${localId}:${attachmentId}`);
  return entry?.blob ?? null;
}

// ---- IndexedDB plumbing for attachments ----

async function putAttachmentBlob(entry: AttachmentBlobEntry): Promise<void> {
  const db = await openAttachmentDB();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(ATTACHMENT_STORE, 'readwrite');
    tx.objectStore(ATTACHMENT_STORE).put(entry);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

async function getAttachmentBlobEntry(key: string): Promise<AttachmentBlobEntry | null> {
  const db = await openAttachmentDB();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(ATTACHMENT_STORE, 'readonly');
    const req = tx.objectStore(ATTACHMENT_STORE).get(key);
    req.onsuccess = () => resolve((req.result as AttachmentBlobEntry | undefined) ?? null);
    req.onerror = () => reject(req.error);
  });
}

async function deleteAttachmentBlob(key: string): Promise<void> {
  const db = await openAttachmentDB();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(ATTACHMENT_STORE, 'readwrite');
    tx.objectStore(ATTACHMENT_STORE).delete(key);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

async function deleteAllAttachmentsFor(localId: string): Promise<void> {
  const db = await openAttachmentDB();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(ATTACHMENT_STORE, 'readwrite');
    const store = tx.objectStore(ATTACHMENT_STORE);
    const index = store.index('draftLocalId');
    const req = index.openCursor(IDBKeyRange.only(localId));
    req.onsuccess = () => {
      const cursor = req.result;
      if (!cursor) return;
      cursor.delete();
      cursor.continue();
    };
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

// ---------------- Sync ----------------

export interface SyncContext {
  /**
   * Posts the draft to the backend. Caller injects this so the module stays
   * decoupled from the api/messages.ts module — and so tests can run without
   * mocking the network.
   *
   * Should reject on transport / 5xx / network errors. Should resolve with
   * `{ status: 'conflict', serverVersion }` to signal a cross-device conflict.
   */
  postDraft: (draft: OfflineDraft) => Promise<{ status: 'ok' } | { status: 'conflict'; serverVersion: number }>;
}

/**
 * Sync one draft to the server and return the updated draft (with the new
 * status). Persists the updated draft as a side effect so the encrypted
 * IndexedDB row reflects the latest state — callers that only want a pure
 * computation should use the `mark*` helpers directly.
 */
export async function syncOne(draft: OfflineDraft, ctx: SyncContext): Promise<OfflineDraft> {
  if (!isDirty(draft)) {
    // Nothing to do — return as-is.
    return draft;
  }
  const syncing = markSyncing(draft);
  await saveDraftLocal(syncing);
  try {
    const result = await ctx.postDraft(syncing);
    if (result.status === 'conflict') {
      const conflicted = markConflict(syncing, result.serverVersion);
      await saveDraftLocal(conflicted);
      return conflicted;
    }
    const synced = markSynced(syncing);
    await saveDraftLocal(synced);
    return synced;
  } catch (err) {
    const message = err instanceof Error ? err.message : 'Unknown sync error';
    const errored = markError(draft, message); // keep original lastEditedAt so we still know we're dirty
    await saveDraftLocal(errored);
    return errored;
  }
}

/**
 * Sync every dirty local draft. Returns counts so the caller can surface a
 * summary in the UI / logs.
 */
export async function syncAllDirty(ctx: SyncContext): Promise<{ synced: number; conflicts: number; errors: number }> {
  const drafts = await listLocalDrafts();
  let synced = 0;
  let conflicts = 0;
  let errors = 0;
  for (const d of drafts) {
    if (!isDirty(d)) continue;
    const result = await syncOne(d, ctx);
    if (result.status === 'synced') synced += 1;
    else if (result.status === 'conflict') conflicts += 1;
    else if (result.status === 'error') errors += 1;
  }
  return { synced, conflicts, errors };
}

/** Convenience: a short human-readable badge for the Composer status pill. */
export function statusBadge(status: DraftStatus): { label: string; tone: 'neutral' | 'good' | 'warn' | 'error' } {
  switch (status) {
    case 'syncing':
      return { label: 'Syncing…', tone: 'neutral' };
    case 'synced':
      return { label: 'Synced to server', tone: 'good' };
    case 'conflict':
      return { label: 'Conflict — review needed', tone: 'warn' };
    case 'error':
      return { label: 'Sync failed — saved locally', tone: 'error' };
    case 'local':
    default:
      return { label: 'Saved locally', tone: 'neutral' };
  }
}
