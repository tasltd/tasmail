/**
 * Background sync queue for offline email actions.
 * Queues actions (send, move, delete, flag) in IndexedDB when offline,
 * replays them when connectivity is restored.
 *
 * Backend routes consumed (TMAIL-207 — dynamic imports, so static analysers
 * such as scripts/trace-check.py won't see them):
 *   - POST /api/messages/schedule   (via api/scheduled.ts → scheduledApi.scheduleSend)
 *   - POST /api/folders/{folder}/messages/{uid}/move    (via api/messages.ts → moveMessage)
 *   - DELETE /api/folders/{folder}/messages/{uid}        (via api/messages.ts → deleteMessage)
 *   - POST /api/folders/{folder}/messages/{uid}/flag    (via api/messages.ts → flagMessage)
 *   - POST /api/drafts                                   (via api/messages.ts → saveDraft)
 *
 * The dynamic-import pattern is intentional: keeps the api/ modules out of
 * the main bundle until the user actually goes offline + queues an action.
 */

const DB_NAME = 'tasmail-sync';
const DB_VERSION = 1;
const STORE_NAME = 'pending-actions';

export type SyncActionType = 'send' | 'move' | 'delete' | 'flag' | 'save-draft';

export interface SyncAction {
  id?: number;
  type: SyncActionType;
  payload: Record<string, unknown>;
  createdAt: number;
  retries: number;
}

function openSyncDB(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);

    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        db.createObjectStore(STORE_NAME, { keyPath: 'id', autoIncrement: true });
      }
    };

    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

// Added: Queue an action for later replay
async function enqueue(type: SyncActionType, payload: Record<string, unknown>): Promise<number> {
  const db = await openSyncDB();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readwrite');
    const store = tx.objectStore(STORE_NAME);
    const action: SyncAction = {
      type,
      payload,
      createdAt: Date.now(),
      retries: 0,
    };
    const request = store.add(action);
    request.onsuccess = () => resolve(request.result as number);
    tx.onerror = () => reject(tx.error);
  });
}

// Added: Get all pending actions ordered by creation time
async function getPending(): Promise<SyncAction[]> {
  const db = await openSyncDB();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readonly');
    const store = tx.objectStore(STORE_NAME);
    const request = store.getAll();
    request.onsuccess = () => resolve(request.result as SyncAction[]);
    request.onerror = () => reject(request.error);
  });
}

// Added: Remove a completed action from the queue
async function remove(id: number): Promise<void> {
  const db = await openSyncDB();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readwrite');
    const store = tx.objectStore(STORE_NAME);
    store.delete(id);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

// Added: Increment retry count for a failed action
async function incrementRetry(id: number): Promise<void> {
  const db = await openSyncDB();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readwrite');
    const store = tx.objectStore(STORE_NAME);
    const getReq = store.get(id);
    getReq.onsuccess = () => {
      const action = getReq.result as SyncAction | undefined;
      if (action) {
        action.retries += 1;
        store.put(action);
      }
      resolve();
    };
    tx.onerror = () => reject(tx.error);
  });
}

// Added: Execute a single queued action via the API
async function executeAction(action: SyncAction): Promise<void> {
  const p = action.payload;

  switch (action.type) {
    case 'send': {
      const { scheduledApi } = await import('../api/scheduled');
      await scheduledApi.scheduleSend({
        to: p.to as string[],
        subject: p.subject as string,
        text_body: p.text_body as string | undefined,
        html_body: p.html_body as string | undefined,
        cc: p.cc as string[] | undefined,
        bcc: p.bcc as string[] | undefined,
        delay_seconds: p.delay_seconds as number | undefined,
      });
      break;
    }
    case 'move': {
      const { moveMessage } = await import('../api/messages');
      await moveMessage(p.folder as string, p.uid as number, p.toFolder as string);
      break;
    }
    case 'delete': {
      const { deleteMessage } = await import('../api/messages');
      await deleteMessage(p.folder as string, p.uid as number);
      break;
    }
    case 'flag': {
      const { flagMessage } = await import('../api/messages');
      await flagMessage(p.folder as string, p.uid as number, p.flag as string, p.add as boolean);
      break;
    }
    case 'save-draft': {
      const { saveDraft } = await import('../api/messages');
      await saveDraft({
        to: p.to as string[],
        subject: p.subject as string,
        cc: p.cc as string[] | undefined,
        html_body: p.html_body as string | undefined,
        text_body: p.text_body as string | undefined,
      });
      break;
    }
    default:
      // NOTE: Unknown action types are removed to prevent queue bloat
      console.warn(`Unknown sync action type: ${action.type}`);
  }
}

const MAX_RETRIES = 3;

// Added: Process all pending actions, removing successful ones
async function processPending(): Promise<{ processed: number; failed: number }> {
  const actions = await getPending();
  let processed = 0;
  let failed = 0;

  for (const action of actions) {
    if (action.retries >= MAX_RETRIES) {
      // NOTE: Permanently failed actions are removed after max retries
      if (action.id != null) await remove(action.id);
      failed += 1;
      continue;
    }

    try {
      await executeAction(action);
      if (action.id != null) await remove(action.id);
      processed += 1;
    } catch {
      if (action.id != null) await incrementRetry(action.id);
      failed += 1;
    }
  }

  return { processed, failed };
}

// Added: Clear all pending actions
async function clearAll(): Promise<void> {
  const db = await openSyncDB();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readwrite');
    tx.objectStore(STORE_NAME).clear();
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

export const backgroundSync = {
  enqueue,
  getPending,
  remove,
  processPending,
  clearAll,
};
