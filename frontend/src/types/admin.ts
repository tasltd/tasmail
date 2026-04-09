export interface Domain {
  id: string;
  name: string;
  active: boolean;
  created_at: string;
  updated_at: string;
}

export interface MailboxInfo {
  id: string;
  domain_id: string;
  username: string;
  display_name: string | null;
  quota_bytes: number;
  active: boolean;
  is_admin: boolean;
  created_at: string;
}

export interface CreateDomainRequest {
  name: string;
}

export interface CreateUserRequest {
  username: string;
  password: string;
  domain_id: string;
  display_name?: string;
  quota_bytes?: number;
}
