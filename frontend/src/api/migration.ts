import type { MigrationJob, CreateImapMigrationRequest, CreateMboxImportRequest } from '../types/migration';
import { apiClient } from './client';

export const migrationApi = {
  list: () => apiClient.get<MigrationJob[]>('/migration'),

  get: (id: string) => apiClient.get<MigrationJob>(`/migration/${id}`),

  startImap: (data: CreateImapMigrationRequest) =>
    apiClient.post<MigrationJob>('/migration/imap', data),

  startMbox: (data: CreateMboxImportRequest) =>
    apiClient.post<MigrationJob>('/migration/mbox', data),

  cancel: (id: string) => apiClient.post(`/migration/${id}/cancel`, {}),
};
