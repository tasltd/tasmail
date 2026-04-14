// Added: DLP API tests for TMAIL-108

import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  listDlpRules,
  createDlpRule,
  updateDlpRule,
  deleteDlpRule,
  listDlpViolations,
  testDlpScan,
} from './dlp';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('dlp API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('listDlpRules', () => {
    it('calls GET /admin/dlp/rules', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      const result = await listDlpRules();
      expect(apiClient.get).toHaveBeenCalledWith('/admin/dlp/rules');
      expect(result).toEqual([]);
    });
  });

  describe('createDlpRule', () => {
    it('calls POST /admin/dlp/rules with rule data', async () => {
      const ruleData = {
        name: 'Credit Card Blocker',
        pattern: '\\b\\d{4}[- ]?\\d{4}[- ]?\\d{4}[- ]?\\d{4}\\b',
        action: 'block' as const,
        severity: 'critical' as const,
      };
      vi.mocked(apiClient.post).mockResolvedValue({ id: 'dlp-1', ...ruleData });

      const result = await createDlpRule(ruleData);
      expect(apiClient.post).toHaveBeenCalledWith('/admin/dlp/rules', ruleData);
      expect(result.name).toBe('Credit Card Blocker');
    });
  });

  describe('updateDlpRule', () => {
    it('calls PUT /admin/dlp/rules/:id with partial update', async () => {
      vi.mocked(apiClient.put).mockResolvedValue({ id: 'dlp-1', active: false });

      await updateDlpRule('dlp-1', { active: false });
      expect(apiClient.put).toHaveBeenCalledWith('/admin/dlp/rules/dlp-1', { active: false });
    });
  });

  describe('deleteDlpRule', () => {
    it('calls DELETE /admin/dlp/rules/:id', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);
      await deleteDlpRule('dlp-1');
      expect(apiClient.delete).toHaveBeenCalledWith('/admin/dlp/rules/dlp-1');
    });
  });

  describe('listDlpViolations', () => {
    it('calls GET /admin/dlp/violations with default pagination', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      const result = await listDlpViolations();
      expect(apiClient.get).toHaveBeenCalledWith('/admin/dlp/violations?limit=50&offset=0');
      expect(result).toEqual([]);
    });

    it('calls GET /admin/dlp/violations with custom pagination', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      await listDlpViolations(25, 100);
      expect(apiClient.get).toHaveBeenCalledWith('/admin/dlp/violations?limit=25&offset=100');
    });
  });

  describe('testDlpScan', () => {
    it('calls POST /admin/dlp/scan with scan data', async () => {
      const scanData = {
        subject: 'Invoice',
        body: 'Card number: 4111-1111-1111-1111',
      };
      const mockMatches = [
        {
          rule_id: '00000000-0000-0000-0000-000000000000',
          rule_name: 'Credit Card Number',
          action: 'block',
          severity: 'critical',
          matched_pattern: '\\d{4}-\\d{4}-\\d{4}-\\d{4}',
          matched_text: '4111-1111-1111-1111',
        },
      ];
      vi.mocked(apiClient.post).mockResolvedValue(mockMatches);

      const result = await testDlpScan(scanData);
      expect(apiClient.post).toHaveBeenCalledWith('/admin/dlp/scan', scanData);
      expect(result).toHaveLength(1);
      expect(result[0].rule_name).toBe('Credit Card Number');
    });
  });
});
