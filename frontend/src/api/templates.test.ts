import { describe, it, expect, vi, beforeEach } from 'vitest';
import { listTemplates, createTemplate, updateTemplate, deleteTemplate, renderTemplate } from './templates';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('templates API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('listTemplates', () => {
    it('calls GET /templates', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      const result = await listTemplates();
      expect(apiClient.get).toHaveBeenCalledWith('/templates');
      expect(result).toEqual([]);
    });

    it('returns array of templates', async () => {
      const mockTemplates = [
        { id: '1', name: 'Welcome', subject: 'Welcome!' },
        { id: '2', name: 'Follow Up', subject: 'Following up' },
      ];
      vi.mocked(apiClient.get).mockResolvedValue(mockTemplates);
      const result = await listTemplates();
      expect(result).toHaveLength(2);
      expect(result[0].name).toBe('Welcome');
    });
  });

  describe('createTemplate', () => {
    it('calls POST /templates with full data', async () => {
      const templateData = {
        name: 'Welcome Email',
        subject: 'Welcome {{first_name}}!',
        body_html: '<h1>Hello {{first_name}}</h1>',
        body_text: 'Hello {{first_name}}',
        merge_fields: ['first_name'],
        category: 'Onboarding',
        is_shared: true,
      };
      vi.mocked(apiClient.post).mockResolvedValue({ id: '1', ...templateData });

      const result = await createTemplate(templateData);
      expect(apiClient.post).toHaveBeenCalledWith('/templates', templateData);
      expect(result.name).toBe('Welcome Email');
    });

    it('creates template with minimal data', async () => {
      const templateData = {
        name: 'Quick Reply',
        subject: 'Re: your message',
        body_html: '<p>Thanks!</p>',
        body_text: 'Thanks!',
      };
      vi.mocked(apiClient.post).mockResolvedValue({ id: '2', ...templateData });

      await createTemplate(templateData);
      expect(apiClient.post).toHaveBeenCalledWith('/templates', templateData);
    });
  });

  describe('updateTemplate', () => {
    it('calls PUT /templates/:id with partial update', async () => {
      vi.mocked(apiClient.put).mockResolvedValue({ id: 'abc', name: 'Updated' });

      await updateTemplate('abc', { name: 'Updated' });
      expect(apiClient.put).toHaveBeenCalledWith('/templates/abc', { name: 'Updated' });
    });

    it('updates multiple fields', async () => {
      const updateData = { subject: 'New Subject', category: 'Marketing' };
      vi.mocked(apiClient.put).mockResolvedValue({ id: 'abc', ...updateData });

      await updateTemplate('abc', updateData);
      expect(apiClient.put).toHaveBeenCalledWith('/templates/abc', updateData);
    });
  });

  describe('deleteTemplate', () => {
    it('calls DELETE /templates/:id', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);
      await deleteTemplate('abc');
      expect(apiClient.delete).toHaveBeenCalledWith('/templates/abc');
    });
  });

  describe('renderTemplate', () => {
    it('calls POST /templates/:id/render with fields', async () => {
      const renderData = { fields: { first_name: 'Alice', company: 'Acme' } };
      const mockResult = {
        subject: 'Welcome Alice!',
        body_html: '<h1>Hello Alice from Acme</h1>',
        body_text: 'Hello Alice from Acme',
      };
      vi.mocked(apiClient.post).mockResolvedValue(mockResult);

      const result = await renderTemplate('tpl-1', renderData);
      expect(apiClient.post).toHaveBeenCalledWith('/templates/tpl-1/render', renderData);
      expect(result.subject).toBe('Welcome Alice!');
      expect(result.body_html).toContain('Acme');
    });

    it('renders with empty fields', async () => {
      const renderData = { fields: {} };
      vi.mocked(apiClient.post).mockResolvedValue({
        subject: 'Hello {{name}}',
        body_html: '<p>Hi {{name}}</p>',
        body_text: 'Hi {{name}}',
      });

      const result = await renderTemplate('tpl-2', renderData);
      expect(apiClient.post).toHaveBeenCalledWith('/templates/tpl-2/render', renderData);
      expect(result.subject).toContain('{{name}}');
    });
  });
});
