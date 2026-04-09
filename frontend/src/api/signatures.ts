import { apiClient } from './client';

export interface Signature {
  id: string;
  mailbox_id: string;
  name: string;
  html_body: string;
  text_body: string;
  is_default: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateSignatureRequest {
  name: string;
  html_body: string;
  text_body: string;
  is_default?: boolean;
}

export interface UpdateSignatureRequest {
  name?: string;
  html_body?: string;
  text_body?: string;
  is_default?: boolean;
}

export async function fetchSignatures(): Promise<Signature[]> {
  return apiClient.get<Signature[]>('/signatures');
}

export async function createSignature(data: CreateSignatureRequest): Promise<Signature> {
  return apiClient.post<Signature>('/signatures', data);
}

export async function updateSignature(id: string, data: UpdateSignatureRequest): Promise<Signature> {
  return apiClient.put<Signature>(`/signatures/${id}`, data);
}

export async function deleteSignature(id: string): Promise<void> {
  await apiClient.delete(`/signatures/${id}`);
}
