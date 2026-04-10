import { describe, it, expect, vi, beforeEach } from 'vitest';
import { migrationApi } from './migration';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
  },
}));

describe('migration API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('lists migration jobs via GET /migration', async () => {
    const mockJobs = [
      { id: '1', type: 'imap', status: 'completed', progress_percent: 100 },
      { id: '2', type: 'mbox', status: 'running', progress_percent: 45 },
    ];
    vi.mocked(apiClient.get).mockResolvedValue(mockJobs);

    const result = await migrationApi.list();

    expect(apiClient.get).toHaveBeenCalledWith('/migration');
    expect(result).toHaveLength(2);
  });

  it('gets migration job by id', async () => {
    const mockJob = { id: 'job-123', type: 'imap', status: 'running', progress_percent: 60 };
    vi.mocked(apiClient.get).mockResolvedValue(mockJob);

    const result = await migrationApi.get('job-123');

    expect(apiClient.get).toHaveBeenCalledWith('/migration/job-123');
    expect(result.status).toBe('running');
  });

  it('starts IMAP migration', async () => {
    const request = {
      source_host: 'imap.old-provider.com',
      source_port: 993,
      source_user: 'user@old.com',
      source_password: 'pass123',
      source_use_ssl: true,
    };
    const mockJob = { id: 'new-job', type: 'imap', status: 'pending', progress_percent: 0 };
    vi.mocked(apiClient.post).mockResolvedValue(mockJob);

    const result = await migrationApi.startImap(request);

    expect(apiClient.post).toHaveBeenCalledWith('/migration/imap', request);
    expect(result.status).toBe('pending');
  });

  it('starts MBOX import', async () => {
    const request = { mbox_file_path: '/tmp/imported.mbox' };
    vi.mocked(apiClient.post).mockResolvedValue({ id: 'mbox-job', type: 'mbox', status: 'pending' });

    await migrationApi.startMbox(request);

    expect(apiClient.post).toHaveBeenCalledWith('/migration/mbox', request);
  });

  it('cancels migration job', async () => {
    vi.mocked(apiClient.post).mockResolvedValue(undefined);

    await migrationApi.cancel('job-to-cancel');

    expect(apiClient.post).toHaveBeenCalledWith('/migration/job-to-cancel/cancel', {});
  });
});
