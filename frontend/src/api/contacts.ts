import { apiClient } from './client';

export interface Contact {
  id: string;
  mailbox_id: string;
  email: string;
  display_name: string | null;
  company: string | null;
  phone: string | null;
  notes: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateContactRequest {
  email: string;
  display_name?: string;
  company?: string;
  phone?: string;
  notes?: string;
}

export interface UpdateContactRequest {
  email?: string;
  display_name?: string;
  company?: string;
  phone?: string;
  notes?: string;
}

export async function fetchContacts(query?: string): Promise<Contact[]> {
  const params = query ? `?q=${encodeURIComponent(query)}` : '';
  return apiClient.get<Contact[]>(`/contacts${params}`);
}

export async function createContact(data: CreateContactRequest): Promise<Contact> {
  return apiClient.post<Contact>('/contacts', data);
}

export async function updateContact(id: string, data: UpdateContactRequest): Promise<Contact> {
  return apiClient.put<Contact>(`/contacts/${id}`, data);
}

export async function deleteContact(id: string): Promise<void> {
  await apiClient.delete(`/contacts/${id}`);
}
