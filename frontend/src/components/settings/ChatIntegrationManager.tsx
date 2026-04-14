// Added: Chat integration management UI for team chat webhooks (TMAIL-129)
// PURPOSE: Allows users to configure webhook integrations with Slack, Teams, Google Chat, Discord, and custom platforms
// EXTERNAL: Uses TanStack Query for data fetching, Zustand for view state

import { useState, type FormEvent } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, Trash2, ArrowLeft, ToggleLeft, ToggleRight, Send, MessageSquare } from 'lucide-react';
import {
  listChatIntegrations,
  createChatIntegration,
  updateChatIntegration,
  deleteChatIntegration,
  testChatIntegration,
} from '../../api/chat-integrations';
import type { ChatIntegration, ChatPlatform } from '../../api/chat-integrations';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

// NOTE: Platform options with display labels for the selector
const PLATFORM_OPTIONS: { value: ChatPlatform; label: string }[] = [
  { value: 'slack', label: 'Slack' },
  { value: 'teams', label: 'Microsoft Teams' },
  { value: 'google_chat', label: 'Google Chat' },
  { value: 'discord', label: 'Discord' },
  { value: 'custom', label: 'Custom' },
];

// Added: Map platform enum to display-friendly badge label
function platformLabel(platform: ChatPlatform): string {
  const found = PLATFORM_OPTIONS.find((p) => p.value === platform);
  return found ? found.label : platform;
}

export function ChatIntegrationManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);
  const [isCreating, setIsCreating] = useState(false);
  const [testingId, setTestingId] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<string | null>(null);

  // Added: Form state for creating new integrations
  const [formPlatform, setFormPlatform] = useState<ChatPlatform>('slack');
  const [formWebhookUrl, setFormWebhookUrl] = useState('');
  const [formChannelName, setFormChannelName] = useState('');
  const [formNotifyReceive, setFormNotifyReceive] = useState(true);
  const [formNotifySend, setFormNotifySend] = useState(false);
  const [formNotifyMention, setFormNotifyMention] = useState(true);
  const [formFilterFrom, setFormFilterFrom] = useState('');
  const [formFilterSubject, setFormFilterSubject] = useState('');

  const { data: integrations, isLoading } = useQuery({
    queryKey: ['chat-integrations'],
    queryFn: listChatIntegrations,
  });

  const createMut = useMutation({
    mutationFn: createChatIntegration,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['chat-integrations'] });
      setIsCreating(false);
      // NOTE: Reset form for next use
      resetForm();
    },
  });

  const toggleMut = useMutation({
    mutationFn: ({ id, active }: { id: string; active: boolean }) =>
      updateChatIntegration(id, { active }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['chat-integrations'] }),
  });

  const deleteMut = useMutation({
    mutationFn: deleteChatIntegration,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['chat-integrations'] }),
  });

  const testMut = useMutation({
    mutationFn: testChatIntegration,
    onSuccess: (result) => {
      setTestResult(result.message);
      setTestingId(null);
    },
    onError: () => {
      setTestResult('Failed to send test notification');
      setTestingId(null);
    },
  });

  // Added: Reset all form fields to defaults
  function resetForm() {
    setFormPlatform('slack');
    setFormWebhookUrl('');
    setFormChannelName('');
    setFormNotifyReceive(true);
    setFormNotifySend(false);
    setFormNotifyMention(true);
    setFormFilterFrom('');
    setFormFilterSubject('');
  }

  const handleCreate = (e: FormEvent) => {
    e.preventDefault();
    createMut.mutate({
      platform: formPlatform,
      webhook_url: formWebhookUrl,
      channel_name: formChannelName || undefined,
      notify_on_receive: formNotifyReceive,
      notify_on_send: formNotifySend,
      notify_on_mention: formNotifyMention,
      filter_from: formFilterFrom || undefined,
      filter_subject: formFilterSubject || undefined,
    });
  };

  const handleTest = (id: string) => {
    setTestingId(id);
    setTestResult(null);
    testMut.mutate(id);
  };

  if (isLoading) return <LoadingSkeleton rows={4} />;

  return (
    <div className="chat-integration-manager" style={{ padding: '16px', maxWidth: '900px' }}>
      <div className="message-view__toolbar">
        <button className="btn btn--icon" onClick={() => setViewMode('list')} title="Back">
          <ArrowLeft size={20} />
        </button>
        <h2 style={{ flex: 1, fontSize: '18px' }}>Chat Integrations</h2>
        <button
          className="btn btn--primary"
          onClick={() => setIsCreating(true)}
          data-testid="add-integration-btn"
        >
          <Plus size={16} /> Add Integration
        </button>
      </div>

      {/* Added: Test result banner */}
      {testResult && (
        <div
          style={{
            marginTop: '12px',
            padding: '8px 12px',
            borderRadius: '6px',
            background: 'var(--color-surface)',
            border: '1px solid var(--color-border)',
            fontSize: '13px',
          }}
          data-testid="test-result"
        >
          {testResult}
          <button
            className="btn btn--icon"
            onClick={() => setTestResult(null)}
            style={{ marginLeft: '8px', fontSize: '12px' }}
          >
            Dismiss
          </button>
        </div>
      )}

      {/* Added: Create integration form */}
      {isCreating && (
        <div
          style={{
            marginTop: '16px',
            padding: '16px',
            border: '1px solid var(--color-border)',
            borderRadius: '8px',
          }}
          data-testid="create-form"
        >
          <h3 style={{ marginBottom: '12px' }}>New Chat Integration</h3>
          <form onSubmit={handleCreate}>
            <div className="composer__field">
              <label>Platform</label>
              <select
                value={formPlatform}
                onChange={(e) => setFormPlatform(e.target.value as ChatPlatform)}
                data-testid="platform-select"
              >
                {PLATFORM_OPTIONS.map((opt) => (
                  <option key={opt.value} value={opt.value}>
                    {opt.label}
                  </option>
                ))}
              </select>
            </div>
            <div className="composer__field">
              <label>Webhook URL</label>
              <input
                value={formWebhookUrl}
                onChange={(e) => setFormWebhookUrl(e.target.value)}
                placeholder="https://hooks.slack.com/services/..."
                required
                type="url"
                data-testid="webhook-url-input"
              />
            </div>
            <div className="composer__field">
              <label>Channel Name</label>
              <input
                value={formChannelName}
                onChange={(e) => setFormChannelName(e.target.value)}
                placeholder="#general (optional)"
                data-testid="channel-name-input"
              />
            </div>
            {/* Added: Notification toggle checkboxes */}
            <div className="composer__field" style={{ flexDirection: 'column', alignItems: 'flex-start' }}>
              <label>Notify On</label>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: '12px', marginTop: '4px' }}>
                <label style={{ display: 'flex', gap: '4px', alignItems: 'center', fontSize: '13px' }}>
                  <input
                    type="checkbox"
                    checked={formNotifyReceive}
                    onChange={(e) => setFormNotifyReceive(e.target.checked)}
                    data-testid="notify-receive"
                  />
                  Receive
                </label>
                <label style={{ display: 'flex', gap: '4px', alignItems: 'center', fontSize: '13px' }}>
                  <input
                    type="checkbox"
                    checked={formNotifySend}
                    onChange={(e) => setFormNotifySend(e.target.checked)}
                    data-testid="notify-send"
                  />
                  Send
                </label>
                <label style={{ display: 'flex', gap: '4px', alignItems: 'center', fontSize: '13px' }}>
                  <input
                    type="checkbox"
                    checked={formNotifyMention}
                    onChange={(e) => setFormNotifyMention(e.target.checked)}
                    data-testid="notify-mention"
                  />
                  Mention
                </label>
              </div>
            </div>
            {/* Added: Optional filter fields */}
            <div className="composer__field">
              <label>Filter From</label>
              <input
                value={formFilterFrom}
                onChange={(e) => setFormFilterFrom(e.target.value)}
                placeholder="sender@example.com (optional)"
              />
            </div>
            <div className="composer__field">
              <label>Filter Subject</label>
              <input
                value={formFilterSubject}
                onChange={(e) => setFormFilterSubject(e.target.value)}
                placeholder="Pattern to match (optional)"
              />
            </div>
            <div className="composer__actions">
              <button type="submit" className="btn btn--primary" data-testid="create-submit">
                Create
              </button>
              <button type="button" className="btn" onClick={() => setIsCreating(false)}>
                Cancel
              </button>
            </div>
          </form>
        </div>
      )}

      {/* Added: Integration list */}
      <div style={{ marginTop: '16px' }}>
        {(!integrations || integrations.length === 0) && !isCreating && (
          <p style={{ color: 'var(--color-text-secondary)', textAlign: 'center', padding: '24px' }}>
            No chat integrations configured. Add one to forward email notifications to your team chat.
          </p>
        )}
        {integrations?.map((integration: ChatIntegration) => (
          <div
            key={integration.id}
            style={{
              padding: '12px',
              borderBottom: '1px solid var(--color-border)',
            }}
            data-testid={`integration-${integration.id}`}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
              <MessageSquare size={18} style={{ color: 'var(--color-text-secondary)' }} />
              <div style={{ flex: 1 }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  {/* Added: Platform badge */}
                  <span
                    style={{
                      fontSize: '11px',
                      padding: '1px 6px',
                      borderRadius: '10px',
                      background: 'var(--color-primary)',
                      color: 'white',
                      fontWeight: 'bold',
                    }}
                    data-testid="platform-badge"
                  >
                    {platformLabel(integration.platform)}
                  </span>
                  {integration.channel_name && (
                    <span style={{ fontSize: '13px', color: 'var(--color-text-secondary)' }}>
                      {integration.channel_name}
                    </span>
                  )}
                  <span
                    style={{
                      fontSize: '11px',
                      padding: '1px 6px',
                      borderRadius: '10px',
                      background: integration.active ? 'green' : 'gray',
                      color: 'white',
                    }}
                  >
                    {integration.active ? 'Active' : 'Inactive'}
                  </span>
                </div>
                <div style={{ fontSize: '12px', color: 'var(--color-text-secondary)', marginTop: '2px' }}>
                  {integration.webhook_url.length > 60
                    ? integration.webhook_url.substring(0, 60) + '...'
                    : integration.webhook_url}
                </div>
              </div>
              {/* Added: Test notification button */}
              <button
                className="btn btn--icon"
                onClick={() => handleTest(integration.id)}
                title="Send test notification"
                disabled={testingId === integration.id}
                data-testid={`test-${integration.id}`}
              >
                <Send size={16} />
              </button>
              {/* Added: Active/inactive toggle */}
              <button
                className="btn btn--icon"
                onClick={() =>
                  toggleMut.mutate({ id: integration.id, active: !integration.active })
                }
                title={integration.active ? 'Deactivate' : 'Activate'}
                data-testid={`toggle-${integration.id}`}
              >
                {integration.active ? <ToggleRight size={20} /> : <ToggleLeft size={20} />}
              </button>
              {/* Added: Delete button */}
              <button
                className="btn btn--icon btn--danger"
                onClick={() => deleteMut.mutate(integration.id)}
                title="Delete"
                data-testid={`delete-${integration.id}`}
              >
                <Trash2 size={16} />
              </button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
