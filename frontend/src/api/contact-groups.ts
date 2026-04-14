// Added: Contact groups API client for TMAIL-119
import { apiClient } from './client';
import type { Contact } from './contacts';

// PURPOSE: Contact group interface matching backend ContactGroup struct
export interface ContactGroup {
  id: string;
  user_id: string;
  name: string;
  color: string | null;
  created_at: string;
}

export interface CreateContactGroupRequest {
  name: string;
  color?: string;
}

export interface UpdateContactGroupRequest {
  name?: string;
  color?: string;
}

export interface ContactGroupMember {
  contact_group_id: string;
  contact_id: string;
}

export interface MergeContactsRequest {
  contact_ids: string[];
}

// PURPOSE: List all contact groups for the current user
export async function listContactGroups(): Promise<ContactGroup[]> {
  return apiClient.get<ContactGroup[]>('/contact-groups');
}

// PURPOSE: Create a new contact group
export async function createContactGroup(data: CreateContactGroupRequest): Promise<ContactGroup> {
  return apiClient.post<ContactGroup>('/contact-groups', data);
}

// PURPOSE: Update an existing contact group
export async function updateContactGroup(id: string, data: UpdateContactGroupRequest): Promise<ContactGroup> {
  return apiClient.put<ContactGroup>(`/contact-groups/${id}`, data);
}

// PURPOSE: Delete a contact group
export async function deleteContactGroup(id: string): Promise<void> {
  await apiClient.delete(`/contact-groups/${id}`);
}

// PURPOSE: Add a contact to a group
export async function addContactToGroup(groupId: string, contactId: string): Promise<ContactGroupMember> {
  return apiClient.post<ContactGroupMember>(`/contact-groups/${groupId}/members`, { contact_id: contactId });
}

// PURPOSE: Remove a contact from a group
export async function removeContactFromGroup(groupId: string, contactId: string): Promise<void> {
  await apiClient.delete(`/contact-groups/${groupId}/members/${contactId}`);
}

// PURPOSE: List contacts belonging to a specific group
export async function listContactsInGroup(groupId: string): Promise<Contact[]> {
  return apiClient.get<Contact[]>(`/contact-groups/${groupId}/contacts`);
}

// PURPOSE: Import contacts from vCard text
export async function importVcard(vcardText: string): Promise<Contact[]> {
  return apiClient.post<Contact[]>('/contacts/import-vcard', { vcard_text: vcardText });
}

// PURPOSE: Export all contacts as vCard text (returns raw text)
export async function exportVcard(): Promise<string> {
  // NOTE: This endpoint returns text/vcard, not JSON — use raw fetch
  const response = await fetch('/api/contacts/export-vcard', {
    headers: {
      'Authorization': `Bearer ${localStorage.getItem('access_token') || ''}`,
    },
  });
  if (!response.ok) {
    throw new Error('Failed to export contacts');
  }
  return response.text();
}

// PURPOSE: Merge duplicate contacts (keep first ID, delete rest)
export async function mergeContacts(contactIds: string[]): Promise<Contact> {
  return apiClient.post<Contact>('/contacts/merge', { contact_ids: contactIds });
}
