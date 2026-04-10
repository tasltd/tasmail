import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fetchSignatures, createSignature, updateSignature, deleteSignature } from './signatures';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('signatures API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('fetchSignatures', () => {
    it('calls GET /signatures', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      const result = await fetchSignatures();
      expect(apiClient.get).toHaveBeenCalledWith('/signatures');
      expect(result).toEqual([]);
    });
  });

  describe('createSignature', () => {
    it('calls POST /signatures with full data', async () => {
      const sig = {
        name: 'Work Signature',
        html_body: '<p>Best regards</p>',
        text_body: 'Best regards',
        is_default: true,
      };
      vi.mocked(apiClient.post).mockResolvedValue({ id: '1', ...sig });

      const result = await createSignature(sig);
      expect(apiClient.post).toHaveBeenCalledWith('/signatures', sig);
      expect(result.name).toBe('Work Signature');
    });

    it('creates signature without is_default', async () => {
      const sig = { name: 'Personal', html_body: '<p>Cheers</p>', text_body: 'Cheers' };
      vi.mocked(apiClient.post).mockResolvedValue({ id: '2', ...sig });

      await createSignature(sig);
      expect(apiClient.post).toHaveBeenCalledWith('/signatures', sig);
    });
  });

  describe('updateSignature', () => {
    it('calls PUT /signatures/:id with partial update', async () => {
      vi.mocked(apiClient.put).mockResolvedValue({ id: 'abc', name: 'Updated' });

      await updateSignature('abc', { name: 'Updated' });
      expect(apiClient.put).toHaveBeenCalledWith('/signatures/abc', { name: 'Updated' });
    });
  });

  describe('deleteSignature', () => {
    it('calls DELETE /signatures/:id', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);
      await deleteSignature('abc');
      expect(apiClient.delete).toHaveBeenCalledWith('/signatures/abc');
    });
  });
});
