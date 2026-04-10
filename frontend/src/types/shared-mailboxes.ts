export interface SharedMailboxView {
  mailbox_id: string;
  username: string;
  display_name: string | null;
  can_read: boolean;
  can_write: boolean;
  can_delete: boolean;
  can_admin: boolean;
}

export interface SharedMailboxAcl {
  id: string;
  mailbox_id: string;
  granted_to: string;
  can_read: boolean;
  can_write: boolean;
  can_delete: boolean;
  can_admin: boolean;
  granted_at: string;
  granted_by: string | null;
}

export interface SharedMailboxAclWithUser {
  id: string;
  mailbox_id: string;
  granted_to: string;
  granted_to_username: string;
  can_read: boolean;
  can_write: boolean;
  can_delete: boolean;
  can_admin: boolean;
  granted_at: string;
}

export interface GrantAccessRequest {
  granted_to: string;
  can_read?: boolean;
  can_write?: boolean;
  can_delete?: boolean;
  can_admin?: boolean;
}
