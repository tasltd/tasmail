export interface DistributionGroup {
  id: string;
  domain_id: string;
  name: string;
  address: string;
  description: string | null;
  owner_mailbox_id: string;
  allow_external: boolean;
  active: boolean;
  created_at: string;
  updated_at: string;
}

export interface GroupMember {
  id: string;
  group_id: string;
  member_address: string;
  mailbox_id: string | null;
  added_at: string;
}

export interface CreateGroupRequest {
  name: string;
  address: string;
  domain_id: string;
  description?: string;
  allow_external?: boolean;
}

export interface UpdateGroupRequest {
  name?: string;
  description?: string;
  allow_external?: boolean;
  active?: boolean;
}

export interface AddMemberRequest {
  member_address: string;
  mailbox_id?: string;
}
