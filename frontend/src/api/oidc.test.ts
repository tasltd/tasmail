// Added: OIDC API tests for TMAIL-99
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  listOidcProviders,
  createOidcProvider,
  updateOidcProvider,
  deleteOidcProvider,
  listLoginProviders,
  getAuthorizeUrl,
} from './oidc';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('oidc API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('listOidcProviders', () => {
    it('calls GET /admin/oidc', async () => {
      const mockProviders = [
        { id: '1', name: 'Google', issuer_url: 'https://accounts.google.com', active: true },
      ];
      vi.mocked(apiClient.get).mockResolvedValue(mockProviders);

      const result = await listOidcProviders();
      expect(apiClient.get).toHaveBeenCalledWith('/admin/oidc');
      expect(result).toHaveLength(1);
      expect(result[0].name).toBe('Google');
    });
  });

  describe('createOidcProvider', () => {
    it('calls POST /admin/oidc with provider data', async () => {
      const createData = {
        name: 'Google',
        issuer_url: 'https://accounts.google.com',
        client_id: '123456.apps.googleusercontent.com',
        client_secret: 'GOCSPX-secret',
        redirect_uri: 'https://mail.example.com/api/auth/oidc/callback',
      };
      vi.mocked(apiClient.post).mockResolvedValue({ id: '2', ...createData, active: true });

      const result = await createOidcProvider(createData);
      expect(apiClient.post).toHaveBeenCalledWith('/admin/oidc', createData);
      expect(result.name).toBe('Google');
    });
  });

  describe('updateOidcProvider', () => {
    it('calls PUT /admin/oidc/:id with partial data', async () => {
      const updateData = { name: 'Updated Google', active: false };
      vi.mocked(apiClient.put).mockResolvedValue({ id: '1', ...updateData });

      const result = await updateOidcProvider('1', updateData);
      expect(apiClient.put).toHaveBeenCalledWith('/admin/oidc/1', updateData);
      expect(result.name).toBe('Updated Google');
    });
  });

  describe('deleteOidcProvider', () => {
    it('calls DELETE /admin/oidc/:id', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);

      await deleteOidcProvider('abc-123');
      expect(apiClient.delete).toHaveBeenCalledWith('/admin/oidc/abc-123');
    });
  });

  describe('listLoginProviders', () => {
    it('calls GET /auth/oidc/providers for public login page', async () => {
      const mockProviders = [
        { id: '1', name: 'Google', icon_url: 'https://cdn.example.com/google.svg', button_label: 'Sign in with Google' },
        { id: '2', name: 'Microsoft', icon_url: null, button_label: 'Sign in with Microsoft' },
      ];
      vi.mocked(apiClient.get).mockResolvedValue(mockProviders);

      const result = await listLoginProviders();
      expect(apiClient.get).toHaveBeenCalledWith('/auth/oidc/providers');
      expect(result).toHaveLength(2);
      expect(result[0].name).toBe('Google');
      expect(result[1].button_label).toBe('Sign in with Microsoft');
    });
  });

  describe('getAuthorizeUrl', () => {
    it('calls GET /auth/oidc/:id/authorize', async () => {
      const mockResponse = {
        authorize_url: 'https://accounts.google.com/authorize?client_id=123&redirect_uri=...',
        state: 'random-state-token',
      };
      vi.mocked(apiClient.get).mockResolvedValue(mockResponse);

      const result = await getAuthorizeUrl('provider-1');
      expect(apiClient.get).toHaveBeenCalledWith('/auth/oidc/provider-1/authorize');
      expect(result.authorize_url).toContain('accounts.google.com');
      expect(result.state).toBe('random-state-token');
    });
  });
});
