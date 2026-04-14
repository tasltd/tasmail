// Added: PST import type definitions for TMAIL-115

export interface PstImport {
  id: string;
  user_id: string;
  filename: string;
  file_size: number;
  status: 'pending' | 'processing' | 'completed' | 'failed';
  target_folder: string;
  messages_found: number | null;
  messages_imported: number | null;
  error_message: string | null;
  started_at: string | null;
  completed_at: string | null;
  created_at: string;
}
