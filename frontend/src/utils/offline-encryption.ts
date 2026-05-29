/**
 * AES-256-GCM encryption for offline-cached content (TMAIL-87).
 *
 * Cached email bodies are sensitive, so they are encrypted at rest in IndexedDB.
 * A 256-bit random key is generated per browser install on first use and persisted
 * in a separate `meta` object store inside the same DB. The key is wiped by
 * `clearSessionKey()` on logout, after which previously-encrypted entries become
 * unreadable garbage (and will be pruned on next sweep).
 *
 * WebCrypto SubtleCrypto is required. The `crypto.subtle` API is available in all
 * supported browsers and in Vitest's jsdom + Node 19+ test environment.
 */

const KEY_DB_NAME = 'tasmail-secrets';
const KEY_DB_VERSION = 1;
const KEY_STORE = 'keys';
const KEY_ID = 'session-aes-256';

export interface EncryptedEnvelope {
  iv: Uint8Array;
  ciphertext: Uint8Array;
}

function openKeyDB(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(KEY_DB_NAME, KEY_DB_VERSION);
    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains(KEY_STORE)) {
        db.createObjectStore(KEY_STORE);
      }
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

async function loadStoredKey(): Promise<CryptoKey | null> {
  const db = await openKeyDB();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(KEY_STORE, 'readonly');
    const req = tx.objectStore(KEY_STORE).get(KEY_ID);
    req.onsuccess = () => resolve((req.result as CryptoKey | undefined) ?? null);
    req.onerror = () => reject(req.error);
  });
}

async function persistKey(key: CryptoKey): Promise<void> {
  const db = await openKeyDB();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(KEY_STORE, 'readwrite');
    tx.objectStore(KEY_STORE).put(key, KEY_ID);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

let cachedKey: CryptoKey | null = null;

// Added: Lazily resolve a session-scoped AES-GCM 256 key, generating + persisting one on first use.
export async function getSessionKey(): Promise<CryptoKey> {
  if (cachedKey) return cachedKey;

  const stored = await loadStoredKey();
  if (stored) {
    cachedKey = stored;
    return stored;
  }

  const fresh = await crypto.subtle.generateKey(
    { name: 'AES-GCM', length: 256 },
    // NOTE: Key is non-extractable so it can't be exfiltrated via XSS reading IndexedDB
    false,
    ['encrypt', 'decrypt'],
  );
  await persistKey(fresh);
  cachedKey = fresh;
  return fresh;
}

// Added: Wipe the session key — called on logout so previously-cached bodies become unreadable.
export async function clearSessionKey(): Promise<void> {
  cachedKey = null;
  const db = await openKeyDB();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(KEY_STORE, 'readwrite');
    tx.objectStore(KEY_STORE).delete(KEY_ID);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

// Added: Encrypt a UTF-8 string. Each call uses a fresh 96-bit IV (GCM requirement).
export async function encryptString(plaintext: string): Promise<EncryptedEnvelope> {
  const key = await getSessionKey();
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const data = new TextEncoder().encode(plaintext);
  const ciphertext = await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, key, data);
  return { iv, ciphertext: new Uint8Array(ciphertext) };
}

// Added: Decrypt to UTF-8 string. Throws if key is missing or envelope is tampered with.
export async function decryptString(envelope: EncryptedEnvelope): Promise<string> {
  const key = await getSessionKey();
  // NOTE: ArrayBuffer slice copies into a fresh non-shared backing buffer so the
  // SubtleCrypto BufferSource overload accepts it under strict ArrayBufferLike.
  const iv = envelope.iv.slice().buffer;
  const ct = envelope.ciphertext.slice().buffer;
  const plain = await crypto.subtle.decrypt({ name: 'AES-GCM', iv }, key, ct);
  return new TextDecoder().decode(plain);
}

// Added: JSON-aware helpers — most callers store objects, not raw strings.
export async function encryptJson<T>(value: T): Promise<EncryptedEnvelope> {
  return encryptString(JSON.stringify(value));
}

export async function decryptJson<T>(envelope: EncryptedEnvelope): Promise<T> {
  return JSON.parse(await decryptString(envelope)) as T;
}
