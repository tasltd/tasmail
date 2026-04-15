// Added: ActiveSync device management API client for TMAIL-130

import { apiClient } from './client';

// Added: ActiveSync device types matching backend structs
export interface ActiveSyncDevice {
  id: string;
  user_id: string;
  device_id: string;
  device_type: string;
  device_name: string | null;
  device_os: string | null;
  last_sync_at: string | null;
  status: 'allowed' | 'blocked' | 'pending' | 'wiped';
  policy_key: string | null;
  created_at: string | null;
  updated_at: string | null;
}

export interface RegisterDeviceRequest {
  device_id: string;
  device_type: string;
  device_name?: string;
  device_os?: string;
}

// Added: ActiveSync policy types matching backend structs
export interface ActiveSyncPolicy {
  id: string;
  name: string;
  require_encryption: boolean;
  max_inactivity_lock_mins: number | null;
  min_password_length: number | null;
  allow_simple_password: boolean;
  max_failed_password_attempts: number | null;
  is_default: boolean;
  created_at: string | null;
}

export interface CreatePolicyRequest {
  name: string;
  require_encryption?: boolean;
  max_inactivity_lock_mins?: number | null;
  min_password_length?: number | null;
  allow_simple_password?: boolean;
  max_failed_password_attempts?: number | null;
  is_default?: boolean;
}

export interface UpdatePolicyRequest {
  name?: string;
  require_encryption?: boolean;
  max_inactivity_lock_mins?: number | null;
  min_password_length?: number | null;
  allow_simple_password?: boolean;
  max_failed_password_attempts?: number | null;
  is_default?: boolean;
}

// --- Device API ---

// PURPOSE: List all ActiveSync devices for the current user
export async function listDevices(): Promise<ActiveSyncDevice[]> {
  return apiClient.get<ActiveSyncDevice[]>('/activesync/devices');
}

// PURPOSE: Register a new ActiveSync device
export async function registerDevice(data: RegisterDeviceRequest): Promise<ActiveSyncDevice> {
  return apiClient.post<ActiveSyncDevice>('/activesync/devices', data);
}

// PURPOSE: Block an ActiveSync device
export async function blockDevice(id: string): Promise<ActiveSyncDevice> {
  return apiClient.post<ActiveSyncDevice>(`/activesync/devices/${id}/block`);
}

// PURPOSE: Allow an ActiveSync device
export async function allowDevice(id: string): Promise<ActiveSyncDevice> {
  return apiClient.post<ActiveSyncDevice>(`/activesync/devices/${id}/allow`);
}

// PURPOSE: Remote wipe an ActiveSync device
export async function wipeDevice(id: string): Promise<ActiveSyncDevice> {
  return apiClient.post<ActiveSyncDevice>(`/activesync/devices/${id}/wipe`);
}

// PURPOSE: Remove a device registration
export async function deleteDevice(id: string): Promise<void> {
  await apiClient.delete(`/activesync/devices/${id}`);
}

// --- Policy API (admin) ---

// PURPOSE: List all ActiveSync policies
export async function listPolicies(): Promise<ActiveSyncPolicy[]> {
  return apiClient.get<ActiveSyncPolicy[]>('/admin/activesync/policies');
}

// PURPOSE: Create a new ActiveSync policy
export async function createPolicy(data: CreatePolicyRequest): Promise<ActiveSyncPolicy> {
  return apiClient.post<ActiveSyncPolicy>('/admin/activesync/policies', data);
}

// PURPOSE: Update an existing ActiveSync policy
export async function updatePolicy(id: string, data: UpdatePolicyRequest): Promise<ActiveSyncPolicy> {
  return apiClient.put<ActiveSyncPolicy>(`/admin/activesync/policies/${id}`, data);
}

// PURPOSE: Delete an ActiveSync policy
export async function deletePolicy(id: string): Promise<void> {
  await apiClient.delete(`/admin/activesync/policies/${id}`);
}
