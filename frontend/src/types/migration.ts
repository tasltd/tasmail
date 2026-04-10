export interface MigrationJob {
  id: string;
  mailbox_id: string;
  job_type: 'imap' | 'mbox';
  status: 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';
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
