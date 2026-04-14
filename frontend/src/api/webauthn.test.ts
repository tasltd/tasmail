// Added: WebAuthn API tests for TMAIL-83
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { webauthnApi, bufferToBase64url, base64urlToBuffer } from './webauthn';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('webauthn API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('starts registration via POST /webauthn/register/begin', async () => {
    const mockResponse = {
      challenge: 'random-challenge-base64url',
      rp: { name: 'TASMail', id: 'mail.example.com' },
      user: { id: 'user-id', name: 'user@example.com', display_name: 'user@example.com' },
      pub_key_cred_params: [{ type: 'public-key', alg: -7 }],
      timeout: 60000,
      attestation: 'none',
    };
    vi.mocked(apiClient.post).mockResolvedValue(mockResponse);

    const result = await webauthnApi.registerBegin();

    expect(apiClient.post).toHaveBeenCalledWith('/webauthn/register/begin');
    expect(result.challenge).toBe('random-challenge-base64url');
    expect(result.rp.name).toBe('TASMail');
    expect(result.pub_key_cred_params).toHaveLength(1);
  });

  it('completes registration via POST /webauthn/register/complete', async () => {
    const mockResponse = { id: 'uuid-123', credential_id: 'cred-abc', name: 'My Key' };
    vi.mocked(apiClient.post).mockResolvedValue(mockResponse);

    const result = await webauthnApi.registerComplete({
      credential_id: 'cred-abc',
      public_key: 'AQIDBA',
      attestation_object: {},
      client_data_json: {},
      name: 'My Key',
    });

    expect(apiClient.post).toHaveBeenCalledWith('/webauthn/register/complete', {
      credential_id: 'cred-abc',
      public_key: 'AQIDBA',
      attestation_object: {},
      client_data_json: {},
      name: 'My Key',
    });
    expect(result.credential_id).toBe('cred-abc');
  });

  it('starts authentication via POST /webauthn/authenticate/begin', async () => {
    const mockResponse = {
      challenge: 'auth-challenge',
      timeout: 60000,
      rp_id: 'mail.example.com',
      allow_credentials: [{ type: 'public-key', id: 'cred-1' }],
    };
    vi.mocked(apiClient.post).mockResolvedValue(mockResponse);

    const result = await webauthnApi.authenticateBegin();

    expect(apiClient.post).toHaveBeenCalledWith('/webauthn/authenticate/begin');
    expect(result.allow_credentials).toHaveLength(1);
  });

  it('completes authentication via POST /webauthn/authenticate/complete', async () => {
    const mockResponse = { verified: true, sign_count: 5 };
    vi.mocked(apiClient.post).mockResolvedValue(mockResponse);

    const result = await webauthnApi.authenticateComplete({
      credential_id: 'cred-abc',
      authenticator_data: {},
      client_data_json: {},
      signature: 'sig-base64',
    });

    expect(apiClient.post).toHaveBeenCalledWith('/webauthn/authenticate/complete', {
      credential_id: 'cred-abc',
      authenticator_data: {},
      client_data_json: {},
      signature: 'sig-base64',
    });
    expect(result.verified).toBe(true);
    expect(result.sign_count).toBe(5);
  });

  it('lists credentials via GET /webauthn/credentials', async () => {
    const mockCredentials = [
      { id: 'uuid-1', credential_id: 'cred-1', name: 'YubiKey', sign_count: 3, created_at: '2026-01-01T00:00:00Z', last_used_at: null },
    ];
    vi.mocked(apiClient.get).mockResolvedValue(mockCredentials);

    const result = await webauthnApi.listCredentials();

    expect(apiClient.get).toHaveBeenCalledWith('/webauthn/credentials');
    expect(result).toHaveLength(1);
    expect(result[0].name).toBe('YubiKey');
  });

  it('deletes a credential via DELETE /webauthn/credentials/{id}', async () => {
    vi.mocked(apiClient.delete).mockResolvedValue(undefined);

    await webauthnApi.deleteCredential('uuid-1');

    expect(apiClient.delete).toHaveBeenCalledWith('/webauthn/credentials/uuid-1');
  });
});

describe('bufferToBase64url', () => {
  it('converts ArrayBuffer to base64url string without padding', () => {
    // NOTE: [1, 2, 3, 4] -> base64 "AQIDBA==" -> base64url "AQIDBA"
    const buffer = new Uint8Array([1, 2, 3, 4]).buffer;
    const result = bufferToBase64url(buffer);
    expect(result).toBe('AQIDBA');
    // Verify no padding characters
    expect(result).not.toContain('=');
  });

  it('handles empty buffer', () => {
    const buffer = new Uint8Array([]).buffer;
    const result = bufferToBase64url(buffer);
    expect(result).toBe('');
  });
});

describe('base64urlToBuffer', () => {
  it('converts base64url string back to ArrayBuffer', () => {
    const buffer = base64urlToBuffer('AQIDBA');
    const bytes = new Uint8Array(buffer);
    expect(Array.from(bytes)).toEqual([1, 2, 3, 4]);
  });

  it('handles base64url with special characters replaced', () => {
    // NOTE: base64url uses - instead of + and _ instead of /
    const original = new Uint8Array([251, 255, 190]).buffer;
    const encoded = bufferToBase64url(original);
    const decoded = base64urlToBuffer(encoded);
    expect(Array.from(new Uint8Array(decoded))).toEqual([251, 255, 190]);
  });
});
