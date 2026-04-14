// Added: LDAP API tests for TMAIL-100
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  listLdapConfigs,
  createLdapConfig,
  updateLdapConfig,
  deleteLdapConfig,
  triggerLdapSync,
  listLdapSyncLogs,
} from './ldap';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('ldap API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('listLdapConfigs', () => {
    it('calls GET /admin/ldap', async () => {
      const mockConfigs = [
        { id: '1', name: 'Corporate AD', server_url: 'ldaps://ad.example.com', active: true },
      ];
      vi.mocked(apiClient.get).mockResolvedValue(mockConfigs);

      const result = await listLdapConfigs();
      expect(apiClient.get).toHaveBeenCalledWith('/admin/ldap');
      expect(result).toHaveLength(1);
      expect(result[0].name).toBe('Corporate AD');
    });
  });

  describe('createLdapConfig', () => {
    it('calls POST /admin/ldap with config data', async () => {
      const createData = {
        name: 'New LDAP',
        server_url: 'ldaps://ldap.example.com:636',
        bind_dn: 'cn=admin,dc=example,dc=com',
        bind_password: 'secret',
        search_base: 'ou=Users,dc=example,dc=com',
      };
      vi.mocked(apiClient.post).mockResolvedValue({ id: '2', ...createData, active: true });

      const result = await createLdapConfig(createData);
      expect(apiClient.post).toHaveBeenCalledWith('/admin/ldap', createData);
      expect(result.name).toBe('New LDAP');
    });
  });

  describe('updateLdapConfig', () => {
    it('calls PUT /admin/ldap/:id with partial data', async () => {
      const updateData = { name: 'Updated LDAP', active: false };
      vi.mocked(apiClient.put).mockResolvedValue({ id: '1', ...updateData });

      const result = await updateLdapConfig('1', updateData);
      expect(apiClient.put).toHaveBeenCalledWith('/admin/ldap/1', updateData);
      expect(result.name).toBe('Updated LDAP');
    });
  });

  describe('deleteLdapConfig', () => {
    it('calls DELETE /admin/ldap/:id', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);

      await deleteLdapConfig('abc-123');
      expect(apiClient.delete).toHaveBeenCalledWith('/admin/ldap/abc-123');
    });
  });

  describe('triggerLdapSync', () => {
    it('calls POST /admin/ldap/:id/sync', async () => {
      const mockLog = {
        id: 'log-1',
        config_id: '1',
        status: 'completed',
        users_created: 5,
        users_updated: 3,
        users_disabled: 1,
      };
      vi.mocked(apiClient.post).mockResolvedValue(mockLog);

      const result = await triggerLdapSync('1');
      expect(apiClient.post).toHaveBeenCalledWith('/admin/ldap/1/sync');
      expect(result.status).toBe('completed');
      expect(result.users_created).toBe(5);
    });
  });

  describe('listLdapSyncLogs', () => {
    it('calls GET /admin/ldap/:id/logs', async () => {
      const mockLogs = [
        { id: 'log-1', config_id: '1', status: 'completed', users_created: 5 },
        { id: 'log-2', config_id: '1', status: 'failed', users_created: 0 },
      ];
      vi.mocked(apiClient.get).mockResolvedValue(mockLogs);

      const result = await listLdapSyncLogs('1');
      expect(apiClient.get).toHaveBeenCalledWith('/admin/ldap/1/logs');
      expect(result).toHaveLength(2);
      expect(result[0].status).toBe('completed');
    });
  });
});
