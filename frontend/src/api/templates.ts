import { apiClient } from './client';

// Added: Email template types and API functions

export interface EmailTemplate {
  id: string;
  mailbox_id: string;
  name: string;
  subject: string;
  body_html: string;
  body_text: string;
  merge_fields: string[];
  category: string | null;
  is_shared: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateTemplateRequest {
  name: string;
  subject: string;
  body_html: string;
  body_text: string;
  merge_fields?: string[];
  category?: string;
  is_shared?: boolean;
}

export interface UpdateTemplateRequest {
  name?: string;
  subject?: string;
  body_html?: string;
  body_text?: string;
  merge_fields?: string[];
  category?: string;
  is_shared?: boolean;
}

export interface RenderTemplateRequest {
  fields: Record<string, string>;
}

export interface RenderResult {
  subject: string;
  body_html: string;
  body_text: string;
}

export async function listTemplates(): Promise<EmailTemplate[]> {
  return apiClient.get<EmailTemplate[]>('/templates');
}

export async function createTemplate(data: CreateTemplateRequest): Promise<EmailTemplate> {
  return apiClient.post<EmailTemplate>('/templates', data);
}

export async function updateTemplate(id: string, data: UpdateTemplateRequest): Promise<EmailTemplate> {
  return apiClient.put<EmailTemplate>(`/templates/${id}`, data);
}

export async function deleteTemplate(id: string): Promise<void> {
  await apiClient.delete(`/templates/${id}`);
}

export async function renderTemplate(id: string, data: RenderTemplateRequest): Promise<RenderResult> {
  return apiClient.post<RenderResult>(`/templates/${id}/render`, data);
}
