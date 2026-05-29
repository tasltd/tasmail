import { describe, it, expect, beforeEach } from 'vitest';
import 'fake-indexeddb/auto';
import {
  encryptString,
  decryptString,
  encryptJson,
  decryptJson,
  getSessionKey,
  clearSessionKey,
} from './offline-encryption';

describe('offline-encryption (TMAIL-87)', () => {
  beforeEach(async () => {
    await clearSessionKey();
  });

  describe('getSessionKey', () => {
    it('generates an AES-GCM key on first use', async () => {
      const key = await getSessionKey();
      expect(key).toBeDefined();
      expect(key.algorithm.name).toBe('AES-GCM');
      expect((key.algorithm as AesKeyAlgorithm).length).toBe(256);
    });

    it('returns the same key across multiple calls in a session', async () => {
      const k1 = await getSessionKey();
      const k2 = await getSessionKey();
      // CryptoKey identity is preserved by the cached reference
      expect(k1).toBe(k2);
    });

    it('non-extractable so it cannot be exported', async () => {
      const key = await getSessionKey();
      expect(key.extractable).toBe(false);
    });
  });

  describe('encryptString / decryptString round-trip', () => {
    it('decrypts back to the same plaintext', async () => {
      const plaintext = 'hello, world — UTF-8 ✓ 你好';
      const env = await encryptString(plaintext);
      const out = await decryptString(env);
      expect(out).toBe(plaintext);
    });

    it('uses a unique IV per encryption (semantic security)', async () => {
      const plaintext = 'same input';
      const a = await encryptString(plaintext);
      const b = await encryptString(plaintext);
      expect(a.iv).not.toEqual(b.iv);
      expect(a.ciphertext).not.toEqual(b.ciphertext);
    });

    it('produces ciphertext different from plaintext bytes', async () => {
      const plaintext = 'plaintext bytes';
      const env = await encryptString(plaintext);
      const ptBytes = new TextEncoder().encode(plaintext);
      expect(env.ciphertext).not.toEqual(ptBytes);
    });

    it('rejects decryption with a tampered ciphertext', async () => {
      const env = await encryptString('tamper me');
      // Flip one byte of ciphertext
      env.ciphertext[0] ^= 0xff;
      await expect(decryptString(env)).rejects.toBeDefined();
    });

    it('handles empty string', async () => {
      const env = await encryptString('');
      expect(await decryptString(env)).toBe('');
    });

    it('handles large strings (1 MB)', async () => {
      const big = 'x'.repeat(1024 * 1024);
      const env = await encryptString(big);
      expect((await decryptString(env)).length).toBe(big.length);
    });
  });

  describe('encryptJson / decryptJson', () => {
    it('round-trips objects', async () => {
      const value = { uid: 42, subject: 'Hi', from: 'a@b.c', flags: ['\\Seen'] };
      const env = await encryptJson(value);
      const back = await decryptJson<typeof value>(env);
      expect(back).toEqual(value);
    });

    it('round-trips arrays', async () => {
      const value = [1, 'two', { three: 3 }];
      const env = await encryptJson(value);
      expect(await decryptJson(env)).toEqual(value);
    });
  });

  describe('clearSessionKey', () => {
    it('makes previously encrypted data undecryptable in the new session', async () => {
      const env = await encryptString('secret');
      await clearSessionKey();
      // A new key is generated on next access — old ciphertext won't decrypt
      await expect(decryptString(env)).rejects.toBeDefined();
    });
  });
});
