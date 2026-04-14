// Added: Branding API tests for TMAIL-111
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { getBranding, updateBranding, resetBranding } from './branding';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('branding API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('getBranding', () => {
    it('calls GET /branding', async () => {
      const mockBranding = {
        id: '1',
        app_name: 'TASMail',
        primary_color: '#2563eb',
        secondary_color: '#1e40af',
        accent_color: '#3b82f6',
      };
      vi.mocked(apiClient.get).mockResolvedValue(mockBranding);

      const result = await getBranding();
      expect(apiClient.get).toHaveBeenCalledWith('/branding');
      expect(result.app_name).toBe('TASMail');
    });
  });

  describe('updateBranding', () => {
    it('calls PUT /admin/branding with partial data', async () => {
      const updateData = { app_name: 'MyMail', primary_color: '#ff0000' };
      vi.mocked(apiClient.put).mockResolvedValue({ id: '1', ...updateData });

      const result = await updateBranding(updateData);
      expect(apiClient.put).toHaveBeenCalledWith('/admin/branding', updateData);
      expect(result.app_name).toBe('MyMail');
    });
  });

  describe('resetBranding', () => {
    it('calls POST /admin/branding/reset', async () => {
      const defaultBranding = { id: '1', app_name: 'TASMail', primary_color: '#2563eb' };
      vi.mocked(apiClient.post).mockResolvedValue(defaultBranding);

      const result = await resetBranding();
      expect(apiClient.post).toHaveBeenCalledWith('/admin/branding/reset');
      expect(result.app_name).toBe('TASMail');
    });
  });
});
