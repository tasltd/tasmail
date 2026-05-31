// TMAIL-345: Modern UI migration API client. Mirrors frontend/src/api/migration.ts
// + frontend/src/api/pst-import.ts so the modern UI Settings → Import pane can
// drive the same /api/migration/{imap,mbox,pst,{id},{id}/cancel,pst/{id}}
// endpoints as the classic SPA — without duplicating type drift between the
// two SPAs.
//
// JSON paths (IMAP + MBOX + cancel + list/get) go through the shared
// apiClient. PST upload is multipart/form-data so it bypasses apiClient and
// uses raw fetch with the access token attached manually — matching the
// classic SPA's pst-import.ts.
import { apiClient } from './client';
import { API_BASE_URL } from './constants';

// ── Migration jobs (IMAP + MBOX) ───────────────────────────────────────────

export type MigrationJobType = 'imap' | 'mbox';
export type MigrationJobStatus =
  | 'pending'
  | 'running'
  | 'completed'
  | 'failed'
  | 'cancelled';

export interface MigrationJob {
  id: string;
  mailbox_id: string;
  job_type: MigrationJobType;
  status: MigrationJobStatus;
  source_host: string | null;
  source_port: number | null;
  source_user: string | null;
  source_use_ssl: boolean | null;
  mbox_file_path: string | null;
  folders_total: number | null;
  folders_done: number | null;
  messages_total: number | null;
  messages_done: number | null;
  bytes_transferred: number | null;
  error_message: string | null;
  started_at: string | null;
  completed_at: string | null;
  created_at: string;
}

export interface CreateImapMigrationRequest {
  source_host: string;
  source_port?: number;
  source_user: string;
  source_password: string;
  source_use_ssl?: boolean;
}

export interface CreateMboxImportRequest {
  mbox_file_path: string;
}

export function listMigrations(): Promise<MigrationJob[]> {
  return apiClient.get<MigrationJob[]>('/migration');
}

export function getMigration(id: string): Promise<MigrationJob> {
  return apiClient.get<MigrationJob>(`/migration/${id}`);
}

export function startImapMigration(
  data: CreateImapMigrationRequest,
): Promise<MigrationJob> {
  return apiClient.post<MigrationJob>('/migration/imap', data);
}

export function startMboxImport(
  data: CreateMboxImportRequest,
): Promise<MigrationJob> {
  return apiClient.post<MigrationJob>('/migration/mbox', data);
}

export function cancelMigration(id: string): Promise<void> {
  return apiClient.post<void>(`/migration/${id}/cancel`, {});
}

// ── PST imports (Outlook .pst, multipart upload) ───────────────────────────

export type PstImportStatus = 'pending' | 'processing' | 'completed' | 'failed';

export interface PstImport {
  id: string;
  user_id: string;
  filename: string;
  file_size: number;
  status: PstImportStatus;
  target_folder: string;
  messages_found: number | null;
  messages_imported: number | null;
  error_message: string | null;
  started_at: string | null;
  completed_at: string | null;
  created_at: string;
}

/**
 * Upload a .pst file via multipart/form-data. Bypasses apiClient because
 * apiClient hardcodes `Content-Type: application/json`, which would break
 * FormData's automatic boundary header. The auth token is attached manually
 * from apiClient.getToken() so the request still rides the same login session.
 */
export async function uploadPst(
  file: File,
  targetFolder?: string,
): Promise<PstImport> {
  const formData = new FormData();
  formData.append('file', file);
  if (targetFolder) {
    formData.append('target_folder', targetFolder);
  }
  const token = apiClient.getToken();
  const response = await fetch(`${API_BASE_URL}/migration/pst/upload`, {
    method: 'POST',
    headers: token ? { Authorization: `Bearer ${token}` } : {},
    body: formData,
  });
  if (!response.ok) {
    const body = await response.text();
    throw new Error(`Upload failed (${response.status}): ${body}`);
  }
  return response.json() as Promise<PstImport>;
}

export function listPstImports(): Promise<PstImport[]> {
  return apiClient.get<PstImport[]>('/migration/pst');
}

export function getPstImport(id: string): Promise<PstImport> {
  return apiClient.get<PstImport>(`/migration/pst/${id}`);
}

export function deletePstImport(id: string): Promise<void> {
  return apiClient.delete<void>(`/migration/pst/${id}`);
}
