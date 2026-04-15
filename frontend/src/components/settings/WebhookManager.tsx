// Added: Webhook management UI for outbound webhook notifications (TMAIL-131)
// PURPOSE: Allows users to create, manage, and monitor webhook endpoints
// EXTERNAL: Uses TanStack Query for data fetching, Zustand for view state

import React from 'react';
import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, Trash2, ArrowLeft, ChevronDown, ChevronRight, ToggleLeft, ToggleRight, Webhook } from 'lucide-react';
import {
  listWebhooks,
  createWebhook,
  updateWebhook,
  deleteWebhook,
  listDeliveries,
} from '../../api/webhooks';
import type { Webhook as WebhookType, WebhookDelivery, WebhookEventType } from '../../api/webhooks';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

// NOTE: All available webhook event types for the checkbox form
const ALL_EVENTS: { value: WebhookEventType; label: string }[] = [
  { value: 'email.received', label: 'Email Received' },
  { value: 'email.sent', label: 'Email Sent' },
  { value: 'email.deleted', label: 'Email Deleted' },
  { value: 'email.moved', label: 'Email Moved' },
  { value: 'email.flagged', label: 'Email Flagged' },
];

// Added: Generate a random hex secret for webhook signing
function generateSecret(): string {
  const array = new Uint8Array(32);
  crypto.getRandomValues(array);
  return Array.from(array, (b) => b.toString(16).padStart(2, '0')).join('');
}

// Added: Delivery log sub-component for a single webhook
function DeliveryLog({ webhookId }: { webhookId: string }) {
  const { data: deliveries, isLoading } = useQuery({
    queryKey: ['webhook-deliveries', webhookId],
    queryFn: () => listDeliveries(webhookId),
  });

  if (isLoading) return <LoadingSkeleton rows={3} />;

  if (!deliveries || deliveries.length === 0) {
    return (
      <p style={{ color: 'var(--color-text-secondary)', fontSize: '13px', padding: '8px 0' }}>
        No deliveries yet.
      </p>
    );
  }

  return (
    <div style={{ marginTop: '8px' }} data-testid="delivery-log">
      <table style={{ width: '100%', fontSize: '12px', borderCollapse: 'collapse' }}>
        <thead>
          <tr style={{ borderBottom: '1px solid var(--color-border)' }}>
            <th style={{ textAlign: 'left', padding: '4px 8px' }}>Event</th>
            <th style={{ textAlign: 'left', padding: '4px 8px' }}>Status</th>
            <th style={{ textAlign: 'left', padding: '4px 8px' }}>Time</th>
            <th style={{ textAlign: 'left', padding: '4px 8px' }}>Result</th>
          </tr>
        </thead>
        <tbody>
          {deliveries.map((delivery: WebhookDelivery) => (
            <tr key={delivery.id} style={{ borderBottom: '1px solid var(--color-border)' }}>
              <td style={{ padding: '4px 8px' }}>{delivery.event}</td>
              <td style={{ padding: '4px 8px' }}>{delivery.response_status ?? '—'}</td>
              <td style={{ padding: '4px 8px' }}>
                {new Date(delivery.delivered_at).toLocaleString()}
              </td>
              <td style={{ padding: '4px 8px' }}>
                <span style={{ color: delivery.success ? 'green' : 'red' }}>
                  {delivery.success ? 'OK' : 'Failed'}
                </span>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export function WebhookManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);
  const [isCreating, setIsCreating] = useState(false);
  const [expandedId, setExpandedId] = useState<string | null>(null);

  // Added: Form state for creating new webhooks
  const [formUrl, setFormUrl] = useState('');
  const [formSecret, setFormSecret] = useState(generateSecret());
  const [formEvents, setFormEvents] = useState<WebhookEventType[]>([]);
  const [formDescription, setFormDescription] = useState('');

  const { data: webhooks, isLoading } = useQuery({
    queryKey: ['webhooks'],
    queryFn: listWebhooks,
  });

  const createMut = useMutation({
    mutationFn: createWebhook,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['webhooks'] });
      setIsCreating(false);
      // NOTE: Reset form for next use
      setFormUrl('');
      setFormSecret(generateSecret());
      setFormEvents([]);
      setFormDescription('');
    },
  });

  const toggleMut = useMutation({
    mutationFn: ({ id, active }: { id: string; active: boolean }) =>
      updateWebhook(id, { active }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['webhooks'] }),
  });

  const deleteMut = useMutation({
    mutationFn: deleteWebhook,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['webhooks'] }),
  });

  // Added: Toggle event checkbox in the form
  const toggleEvent = (event: WebhookEventType) => {
    setFormEvents((prev) =>
      prev.includes(event) ? prev.filter((e) => e !== event) : [...prev, event],
    );
  };

  const handleCreate = (e: React.FormEvent) => {
    e.preventDefault();
    createMut.mutate({
      url: formUrl,
      secret: formSecret,
      events: formEvents,
      description: formDescription || undefined,
    });
  };

  if (isLoading) return <LoadingSkeleton rows={4} />;

  return (
    <div className="webhook-manager" style={{ padding: '16px', maxWidth: '900px' }}>
      <div className="message-view__toolbar">
        <button className="btn btn--icon" onClick={() => setViewMode('list')} title="Back">
          <ArrowLeft size={20} />
        </button>
        <h2 style={{ flex: 1, fontSize: '18px' }}>Webhooks</h2>
        <button
          className="btn btn--primary"
          onClick={() => setIsCreating(true)}
        >
          <Plus size={16} /> Add Webhook
        </button>
      </div>

      {/* Added: Create webhook form */}
      {isCreating && (
        <div
          style={{
            marginTop: '16px',
            padding: '16px',
            border: '1px solid var(--color-border)',
            borderRadius: '8px',
          }}
        >
          <h3 style={{ marginBottom: '12px' }}>New Webhook</h3>
          <form onSubmit={handleCreate}>
            <div className="composer__field">
              <label>URL</label>
              <input
                value={formUrl}
                onChange={(e) => setFormUrl(e.target.value)}
                placeholder="https://example.com/webhook"
                required
                type="url"
              />
            </div>
            <div className="composer__field">
              <label>Secret</label>
              <input
                value={formSecret}
                onChange={(e) => setFormSecret(e.target.value)}
                placeholder="Signing secret"
                required
              />
            </div>
            <div className="composer__field" style={{ flexDirection: 'column', alignItems: 'flex-start' }}>
              <label>Events</label>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px', marginTop: '4px' }}>
                {ALL_EVENTS.map((evt) => (
                  <label
                    key={evt.value}
                    style={{ display: 'flex', gap: '4px', alignItems: 'center', fontSize: '13px' }}
                  >
                    <input
                      type="checkbox"
                      checked={formEvents.includes(evt.value)}
                      onChange={() => toggleEvent(evt.value)}
                      data-testid={`event-${evt.value}`}
                    />
                    {evt.label}
                  </label>
                ))}
              </div>
            </div>
            <div className="composer__field">
              <label>Description</label>
              <input
                value={formDescription}
                onChange={(e) => setFormDescription(e.target.value)}
                placeholder="Optional description"
              />
            </div>
            <div className="composer__actions">
              <button type="submit" className="btn btn--primary" disabled={formEvents.length === 0}>
                Create
              </button>
              <button type="button" className="btn" onClick={() => setIsCreating(false)}>
                Cancel
              </button>
            </div>
          </form>
        </div>
      )}

      {/* Added: Webhook list */}
      <div style={{ marginTop: '16px' }}>
        {(!webhooks || webhooks.length === 0) && !isCreating && (
          <p style={{ color: 'var(--color-text-secondary)', textAlign: 'center', padding: '24px' }}>
            No webhooks configured. Add one to receive notifications for email events.
          </p>
        )}
        {webhooks?.map((webhook: WebhookType) => (
          <div
            key={webhook.id}
            style={{
              padding: '12px',
              borderBottom: '1px solid var(--color-border)',
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
              {/* Added: Expand/collapse toggle for delivery log */}
              <button
                className="btn btn--icon"
                onClick={() =>
                  setExpandedId(expandedId === webhook.id ? null : webhook.id)
                }
                title="Toggle deliveries"
              >
                {expandedId === webhook.id ? (
                  <ChevronDown size={16} />
                ) : (
                  <ChevronRight size={16} />
                )}
              </button>
              <Webhook size={18} style={{ color: 'var(--color-text-secondary)' }} />
              <div style={{ flex: 1 }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  <strong style={{ fontSize: '14px' }}>{webhook.url}</strong>
                  <span
                    style={{
                      fontSize: '11px',
                      padding: '1px 6px',
                      borderRadius: '10px',
                      background: webhook.active ? 'green' : 'gray',
                      color: 'white',
                    }}
                  >
                    {webhook.active ? 'Active' : 'Inactive'}
                  </span>
                </div>
                <div style={{ fontSize: '12px', color: 'var(--color-text-secondary)', marginTop: '2px' }}>
                  {webhook.events.length} event{webhook.events.length !== 1 ? 's' : ''}
                  {webhook.last_triggered_at && (
                    <> &middot; Last triggered {new Date(webhook.last_triggered_at).toLocaleDateString()}</>
                  )}
                  {webhook.failure_count > 0 && (
                    <span style={{ color: 'red' }}> &middot; {webhook.failure_count} failures</span>
                  )}
                </div>
              </div>
              {/* Added: Active/inactive toggle */}
              <button
                className="btn btn--icon"
                onClick={() =>
                  toggleMut.mutate({ id: webhook.id, active: !webhook.active })
                }
                title={webhook.active ? 'Deactivate' : 'Activate'}
                data-testid={`toggle-${webhook.id}`}
              >
                {webhook.active ? <ToggleRight size={20} /> : <ToggleLeft size={20} />}
              </button>
              {/* Added: Delete button */}
              <button
                className="btn btn--icon btn--danger"
                onClick={() => deleteMut.mutate(webhook.id)}
                title="Delete"
              >
                <Trash2 size={16} />
              </button>
            </div>
            {/* Added: Expanded delivery log */}
            {expandedId === webhook.id && <DeliveryLog webhookId={webhook.id} />}
          </div>
        ))}
      </div>
    </div>
  );
}
