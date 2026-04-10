import { useState, useEffect } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plane, Save } from 'lucide-react';
import { autoReplyApi } from '../../api/auto-reply';
import type { AutoReplyRule, UpsertAutoReply } from '../../api/auto-reply';

export function VacationResponder() {
  const queryClient = useQueryClient();
  const [error, setError] = useState('');
  const [saved, setSaved] = useState(false);

  const [enabled, setEnabled] = useState(false);
  const [subject, setSubject] = useState('Out of Office');
  const [bodyText, setBodyText] = useState('');
  const [startDate, setStartDate] = useState('');
  const [endDate, setEndDate] = useState('');
  const [replyToAll, setReplyToAll] = useState(false);
  const [excludeLists, setExcludeLists] = useState(true);

  const { data: rule } = useQuery<AutoReplyRule | null>({
    queryKey: ['auto-reply'],
    queryFn: autoReplyApi.get,
  });

  // Populate form when data loads
  useEffect(() => {
    if (rule) {
      setEnabled(rule.enabled);
      setSubject(rule.subject);
      setBodyText(rule.body_text);
      setStartDate(rule.start_date ? rule.start_date.slice(0, 16) : '');
      setEndDate(rule.end_date ? rule.end_date.slice(0, 16) : '');
      setReplyToAll(rule.reply_to_all);
      setExcludeLists(rule.exclude_lists);
    }
  }, [rule]);

  const saveMutation = useMutation({
    mutationFn: (data: UpsertAutoReply) => autoReplyApi.set(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['auto-reply'] });
      setSaved(true);
      setError('');
      setTimeout(() => setSaved(false), 2000);
    },
    onError: (err: Error) => setError(err.message),
  });

  const handleSave = () => {
    saveMutation.mutate({
      enabled,
      subject,
      body_text: bodyText,
      start_date: startDate ? new Date(startDate).toISOString() : undefined,
      end_date: endDate ? new Date(endDate).toISOString() : undefined,
      reply_to_all: replyToAll,
      exclude_lists: excludeLists,
    });
  };

  return (
    <div style={{ padding: '24px', maxWidth: '600px' }}>
      <h2 style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '16px' }}>
        <Plane size={24} />
        Vacation Responder
      </h2>

      {error && (
        <div style={{ padding: '8px 12px', background: 'var(--color-error-bg, #ffeaea)', color: 'var(--color-error, #dc3545)', borderRadius: '4px', marginBottom: '12px' }}>
          {error}
        </div>
      )}

      <div style={{ marginBottom: '16px' }}>
        <label style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer' }}>
          <input
            type="checkbox"
            checked={enabled}
            onChange={(e) => setEnabled(e.target.checked)}
          />
          <strong>Enable vacation responder</strong>
        </label>
      </div>

      <div style={{ display: 'flex', flexDirection: 'column', gap: '12px', opacity: enabled ? 1 : 0.5 }}>
        <div>
          <label style={{ display: 'block', fontSize: '13px', marginBottom: '4px' }}>Subject</label>
          <input
            type="text"
            value={subject}
            onChange={(e) => setSubject(e.target.value)}
            disabled={!enabled}
            style={{ width: '100%', padding: '8px 12px' }}
          />
        </div>

        <div>
          <label style={{ display: 'block', fontSize: '13px', marginBottom: '4px' }}>Message</label>
          <textarea
            value={bodyText}
            onChange={(e) => setBodyText(e.target.value)}
            disabled={!enabled}
            rows={6}
            style={{ width: '100%', padding: '8px 12px', resize: 'vertical' }}
            placeholder="I'm currently out of the office..."
          />
        </div>

        <div style={{ display: 'flex', gap: '12px' }}>
          <div style={{ flex: 1 }}>
            <label style={{ display: 'block', fontSize: '13px', marginBottom: '4px' }}>Start date (optional)</label>
            <input
              type="datetime-local"
              value={startDate}
              onChange={(e) => setStartDate(e.target.value)}
              disabled={!enabled}
              style={{ width: '100%', padding: '8px 12px' }}
            />
          </div>
          <div style={{ flex: 1 }}>
            <label style={{ display: 'block', fontSize: '13px', marginBottom: '4px' }}>End date (optional)</label>
            <input
              type="datetime-local"
              value={endDate}
              onChange={(e) => setEndDate(e.target.value)}
              disabled={!enabled}
              style={{ width: '100%', padding: '8px 12px' }}
            />
          </div>
        </div>

        <div style={{ display: 'flex', gap: '16px' }}>
          <label style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '13px', cursor: 'pointer' }}>
            <input type="checkbox" checked={excludeLists} onChange={(e) => setExcludeLists(e.target.checked)} disabled={!enabled} />
            Skip mailing lists
          </label>
          <label style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '13px', cursor: 'pointer' }}>
            <input type="checkbox" checked={replyToAll} onChange={(e) => setReplyToAll(e.target.checked)} disabled={!enabled} />
            Reply to all recipients
          </label>
        </div>
      </div>

      <div style={{ marginTop: '16px' }}>
        <button
          className="btn btn--primary"
          onClick={handleSave}
          disabled={saveMutation.isPending}
        >
          <Save size={16} />
          {saved ? 'Saved!' : saveMutation.isPending ? 'Saving...' : 'Save Settings'}
        </button>
      </div>
    </div>
  );
}
