// Added: ActiveSync API client tests for TMAIL-130

import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  listDevices,
  registerDevice,
  blockDevice,
  allowDevice,
  wipeDevice,
  deleteDevice,
  listPolicies,
  createPolicy,
  updatePolicy,
  deletePolicy,
} from './activesync';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('activesync API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // --- Device API tests ---

  describe('listDevices', () => {
    it('calls GET /activesync/devices', async () => {
      const mockDevices = [
        {
          id: 'dev-1',
          user_id: 'user-1',
          device_id: 'IPHONE123',
          device_type: 'iPhone',
          device_name: 'My iPhone',
          device_os: 'iOS 18',
          last_sync_at: '2026-04-14T10:00:00Z',
          status: 'allowed',
          policy_key: null,
          created_at: '2026-04-14T00:00:00Z',
          updated_at: '2026-04-14T00:00:00Z',
        },
      ];
      vi.mocked(apiClient.get).mockResolvedValue(mockDevices);

      const result = await listDevices();
      expect(apiClient.get).toHaveBeenCalledWith('/activesync/devices');
      expect(result).toHaveLength(1);
      expect(result[0].device_type).toBe('iPhone');
    });

    it('returns empty array when no devices', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);

      const result = await listDevices();
      expect(result).toEqual([]);
    });
  });

  describe('registerDevice', () => {
    it('calls POST /activesync/devices with full data', async () => {
      const requestData = {
        device_id: 'ANDROID456',
        device_type: 'Android',
        device_name: 'Pixel 9',
        device_os: 'Android 15',
      };
      vi.mocked(apiClient.post).mockResolvedValue({
        id: 'dev-2',
        ...requestData,
        status: 'allowed',
      });

      const result = await registerDevice(requestData);
      expect(apiClient.post).toHaveBeenCalledWith('/activesync/devices', requestData);
      expect(result.device_type).toBe('Android');
    });

    it('calls POST /activesync/devices with minimal data', async () => {
      const requestData = {
        device_id: 'WIN789',
        device_type: 'WindowsMail',
      };
      vi.mocked(apiClient.post).mockResolvedValue({
        id: 'dev-3',
        ...requestData,
        device_name: null,
        device_os: null,
        status: 'allowed',
      });

      const result = await registerDevice(requestData);
      expect(apiClient.post).toHaveBeenCalledWith('/activesync/devices', requestData);
      expect(result.device_name).toBeNull();
    });
  });

  describe('blockDevice', () => {
    it('calls POST /activesync/devices/{id}/block', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({
        id: 'dev-1',
        status: 'blocked',
      });

      const result = await blockDevice('dev-1');
      expect(apiClient.post).toHaveBeenCalledWith('/activesync/devices/dev-1/block');
      expect(result.status).toBe('blocked');
    });
  });

  describe('allowDevice', () => {
    it('calls POST /activesync/devices/{id}/allow', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({
        id: 'dev-1',
        status: 'allowed',
      });

      const result = await allowDevice('dev-1');
      expect(apiClient.post).toHaveBeenCalledWith('/activesync/devices/dev-1/allow');
      expect(result.status).toBe('allowed');
    });
  });

  describe('wipeDevice', () => {
    it('calls POST /activesync/devices/{id}/wipe', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({
        id: 'dev-1',
        status: 'wiped',
      });

      const result = await wipeDevice('dev-1');
      expect(apiClient.post).toHaveBeenCalledWith('/activesync/devices/dev-1/wipe');
      expect(result.status).toBe('wiped');
    });
  });

  describe('deleteDevice', () => {
    it('calls DELETE /activesync/devices/{id}', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);

      await deleteDevice('dev-1');
      expect(apiClient.delete).toHaveBeenCalledWith('/activesync/devices/dev-1');
    });
  });

  // --- Policy API tests ---

  describe('listPolicies', () => {
    it('calls GET /admin/activesync/policies', async () => {
      const mockPolicies = [
        {
          id: 'pol-1',
          name: 'Default',
          require_encryption: true,
          max_inactivity_lock_mins: 5,
          min_password_length: 4,
          allow_simple_password: false,
          max_failed_password_attempts: 10,
          is_default: true,
          created_at: '2026-04-14T00:00:00Z',
        },
      ];
      vi.mocked(apiClient.get).mockResolvedValue(mockPolicies);

      const result = await listPolicies();
      expect(apiClient.get).toHaveBeenCalledWith('/admin/activesync/policies');
      expect(result).toHaveLength(1);
      expect(result[0].is_default).toBe(true);
    });
  });

  describe('createPolicy', () => {
    it('calls POST /admin/activesync/policies with data', async () => {
      const policyData = {
        name: 'Strict Policy',
        require_encryption: true,
        min_password_length: 8,
        is_default: false,
      };
      vi.mocked(apiClient.post).mockResolvedValue({
        id: 'pol-2',
        ...policyData,
        allow_simple_password: false,
        max_inactivity_lock_mins: null,
        max_failed_password_attempts: null,
      });

      const result = await createPolicy(policyData);
      expect(apiClient.post).toHaveBeenCalledWith('/admin/activesync/policies', policyData);
      expect(result.name).toBe('Strict Policy');
    });
  });

  describe('updatePolicy', () => {
    it('calls PUT /admin/activesync/policies/{id} with data', async () => {
      const updateData = { name: 'Renamed Policy', is_default: true };
      vi.mocked(apiClient.put).mockResolvedValue({
        id: 'pol-1',
        name: 'Renamed Policy',
        is_default: true,
      });

      const result = await updatePolicy('pol-1', updateData);
      expect(apiClient.put).toHaveBeenCalledWith('/admin/activesync/policies/pol-1', updateData);
      expect(result.name).toBe('Renamed Policy');
    });
  });

  describe('deletePolicy', () => {
    it('calls DELETE /admin/activesync/policies/{id}', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);

      await deletePolicy('pol-1');
      expect(apiClient.delete).toHaveBeenCalledWith('/admin/activesync/policies/pol-1');
    });
  });
});
