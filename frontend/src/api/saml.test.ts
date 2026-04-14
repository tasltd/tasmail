// Added: SAML API tests for TMAIL-101
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  listSamlConfigs,
  createSamlConfig,
  updateSamlConfig,
  deleteSamlConfig,
  getSamlLoginUrl,
} from './saml';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('saml API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('listSamlConfigs', () => {
    it('calls GET /admin/saml', async () => {
      const mockConfigs = [
        { id: '1', name: 'Okta SSO', entity_id: 'https://okta.example.com', active: true },
      ];
      vi.mocked(apiClient.get).mockResolvedValue(mockConfigs);

      const result = await listSamlConfigs();
      expect(apiClient.get).toHaveBeenCalledWith('/admin/saml');
      expect(result).toHaveLength(1);
      expect(result[0].name).toBe('Okta SSO');
    });
  });

  describe('createSamlConfig', () => {
    it('calls POST /admin/saml with config data', async () => {
      const createData = {
        name: 'Azure AD',
        entity_id: 'https://sts.windows.net/tenant-id/',
        sso_url: 'https://login.microsoftonline.com/tenant-id/saml2',
        certificate: 'MIICpDCCAYwCCQ...',
      };
      vi.mocked(apiClient.post).mockResolvedValue({ id: '2', ...createData, active: true });

      const result = await createSamlConfig(createData);
      expect(apiClient.post).toHaveBeenCalledWith('/admin/saml', createData);
      expect(result.name).toBe('Azure AD');
    });
  });

  describe('updateSamlConfig', () => {
    it('calls PUT /admin/saml/:id with partial data', async () => {
      const updateData = { name: 'Updated SSO', active: false };
      vi.mocked(apiClient.put).mockResolvedValue({ id: '1', ...updateData });

      const result = await updateSamlConfig('1', updateData);
      expect(apiClient.put).toHaveBeenCalledWith('/admin/saml/1', updateData);
      expect(result.name).toBe('Updated SSO');
    });
  });

  describe('deleteSamlConfig', () => {
    it('calls DELETE /admin/saml/:id', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);

      await deleteSamlConfig('abc-123');
      expect(apiClient.delete).toHaveBeenCalledWith('/admin/saml/abc-123');
    });
  });

  describe('getSamlLoginUrl', () => {
    it('calls GET /auth/saml/:id/login and returns redirect URL', async () => {
      const mockResponse = {
        redirect_url: 'https://idp.example.com/sso?SAMLRequest=encoded_data',
      };
      vi.mocked(apiClient.get).mockResolvedValue(mockResponse);

      const result = await getSamlLoginUrl('cfg-1');
      expect(apiClient.get).toHaveBeenCalledWith('/auth/saml/cfg-1/login');
      expect(result.redirect_url).toContain('SAMLRequest');
    });
  });
});
