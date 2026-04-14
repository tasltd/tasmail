// Added: Webhook API client for outbound webhook management (TMAIL-131)

import { apiClient } from './client';

// Added: Webhook event types matching backend enum
export type WebhookEventType =
  | 'email.received'
  | 'email.sent'
  | 'email.deleted'
  | 'email.moved'
  | 'email.flagged';

export interface Webhook {
  id: string;
  user_id: string;
  url: string;
  secret: string;
  events: WebhookEventType[];
  active: boolean;
  description: string | null;
  created_at: string;
  updated_at: string;
  last_triggered_at: string | null;
  failure_count: number;
}

export interface WebhookDelivery {
  id: string;
  webhook_id: string;
  event: WebhookEventType;
  payload: Record<string, unknown>;
  response_status: number | null;
  response_body: string | null;
  delivered_at: string;
  success: boolean;
}

export interface CreateWebhookRequest {
  url: string;
  secret: string;
  events: WebhookEventType[];
  description?: string;
}

export interface UpdateWebhookRequest {
  url?: string;
  secret?: string;
  events?: WebhookEventType[];
  active?: boolean;
  description?: string;
}

// PURPOSE: List all webhooks for the current user
export async function listWebhooks(): Promise<Webhook[]> {
  return apiClient.get<Webhook[]>('/webhooks');
}

// PURPOSE: Create a new webhook endpoint
export async function createWebhook(data: CreateWebhookRequest): Promise<Webhook> {
  return apiClient.post<Webhook>('/webhooks', data);
}

// PURPOSE: Get a single webhook by ID
export async function getWebhook(id: string): Promise<Webhook> {
  return apiClient.get<Webhook>(`/webhooks/${id}`);
}

// PURPOSE: Update an existing webhook
export async function updateWebhook(id: string, data: UpdateWebhookRequest): Promise<Webhook> {
  return apiClient.put<Webhook>(`/webhooks/${id}`, data);
}

// PURPOSE: Delete a webhook and all its delivery records
export async function deleteWebhook(id: string): Promise<void> {
  await apiClient.delete(`/webhooks/${id}`);
}

// PURPOSE: List recent delivery attempts for a webhook
export async function listDeliveries(webhookId: string): Promise<WebhookDelivery[]> {
  return apiClient.get<WebhookDelivery[]>(`/webhooks/${webhookId}/deliveries`);
}
