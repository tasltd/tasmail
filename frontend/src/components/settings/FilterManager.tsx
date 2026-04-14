import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, Trash2, Edit2, ArrowLeft, ArrowUp, ArrowDown, Power } from 'lucide-react';
import {
  listFilters,
  createFilter,
  updateFilter,
  deleteFilter,
  reorderFilters,
} from '../../api/filters';
import type { SieveRule, RuleCondition, RuleAction, CreateFilterRequest } from '../../api/filters';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

// Added: available field/operator/action options for the form
const FIELDS = [
  { value: 'from', label: 'From' },
  { value: 'to', label: 'To' },
  { value: 'cc', label: 'CC' },
  { value: 'subject', label: 'Subject' },
  { value: 'body', label: 'Body' },
] as const;

const OPERATORS = [
  { value: 'contains', label: 'contains' },
  { value: 'not_contains', label: 'does not contain' },
  { value: 'equals', label: 'equals' },
  { value: 'starts_with', label: 'starts with' },
  { value: 'ends_with', label: 'ends with' },
] as const;

const ACTION_TYPES = [
  { value: 'move', label: 'Move to folder', needsTarget: true },
  { value: 'copy', label: 'Copy to folder', needsTarget: true },
  { value: 'delete', label: 'Delete', needsTarget: false },
  { value: 'mark_read', label: 'Mark as read', needsTarget: false },
  { value: 'mark_flagged', label: 'Flag', needsTarget: false },
  { value: 'forward', label: 'Forward to', needsTarget: true },
  { value: 'reject', label: 'Reject', needsTarget: false },
] as const;

function ConditionRow({
  condition,
  onChange,
  onRemove,
}: {
  condition: RuleCondition;
  onChange: (c: RuleCondition) => void;
  onRemove: () => void;
}) {
  return (
    <div className="filter-condition" style={{ display: 'flex', gap: '8px', alignItems: 'center', marginBottom: '8px' }}>
      <select
        value={condition.field}
        onChange={(e) => onChange({ ...condition, field: e.target.value as RuleCondition['field'] })}
      >
        {FIELDS.map((f) => (
          <option key={f.value} value={f.value}>{f.label}</option>
        ))}
      </select>
      <select
        value={condition.operator}
        onChange={(e) => onChange({ ...condition, operator: e.target.value as RuleCondition['operator'] })}
      >
        {OPERATORS.map((o) => (
          <option key={o.value} value={o.value}>{o.label}</option>
        ))}
      </select>
      <input
        type="text"
        value={condition.value}
        onChange={(e) => onChange({ ...condition, value: e.target.value })}
        placeholder="Value"
        required
        style={{ flex: 1 }}
      />
      <button type="button" onClick={onRemove} className="btn btn--icon" title="Remove condition">
        <Trash2 size={14} />
      </button>
    </div>
  );
}

function ActionRow({
  action,
  onChange,
  onRemove,
}: {
  action: RuleAction;
  onChange: (a: RuleAction) => void;
  onRemove: () => void;
}) {
  const actionDef = ACTION_TYPES.find((a) => a.value === action.action_type);
  return (
    <div className="filter-action" style={{ display: 'flex', gap: '8px', alignItems: 'center', marginBottom: '8px' }}>
      <select
        value={action.action_type}
        onChange={(e) => onChange({ ...action, action_type: e.target.value as RuleAction['action_type'], target: undefined })}
      >
        {ACTION_TYPES.map((a) => (
          <option key={a.value} value={a.value}>{a.label}</option>
        ))}
      </select>
      {actionDef?.needsTarget && (
        <input
          type="text"
          value={action.target || ''}
          onChange={(e) => onChange({ ...action, target: e.target.value })}
          placeholder={action.action_type === 'forward' ? 'Email address' : 'Folder name'}
          required
          style={{ flex: 1 }}
        />
      )}
      <button type="button" onClick={onRemove} className="btn btn--icon" title="Remove action">
        <Trash2 size={14} />
      </button>
    </div>
  );
}

function FilterEditor({
  rule,
  onSave,
  onCancel,
}: {
  rule?: SieveRule;
  onSave: (data: CreateFilterRequest) => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState(rule?.name || '');
  const [matchMode, setMatchMode] = useState<'all' | 'any'>(rule?.match_mode || 'all');
  const [conditions, setConditions] = useState<RuleCondition[]>(
    rule?.conditions || [{ field: 'from', operator: 'contains', value: '' }]
  );
  const [actions, setActions] = useState<RuleAction[]>(
    rule?.actions || [{ action_type: 'move', target: '' }]
  );
  const [stopProcessing, setStopProcessing] = useState(rule?.stop_processing ?? true);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    onSave({
      name,
      match_mode: matchMode,
      conditions,
      actions,
      stop_processing: stopProcessing,
      enabled: rule?.enabled ?? true,
    });
  };

  const updateCondition = (index: number, c: RuleCondition) => {
    const updated = [...conditions];
    updated[index] = c;
    setConditions(updated);
  };

  const updateAction = (index: number, a: RuleAction) => {
    const updated = [...actions];
    updated[index] = a;
    setActions(updated);
  };

  return (
    <form className="filter-editor" onSubmit={handleSubmit}>
      <div className="composer__field">
        <label>Rule Name</label>
        <input value={name} onChange={(e) => setName(e.target.value)} placeholder="e.g., Move newsletters" required />
      </div>

      <div style={{ margin: '16px 0' }}>
        <label style={{ fontWeight: 600 }}>When </label>
        <select value={matchMode} onChange={(e) => setMatchMode(e.target.value as 'all' | 'any')}>
          <option value="all">ALL conditions match</option>
          <option value="any">ANY condition matches</option>
        </select>
      </div>

      <fieldset style={{ border: '1px solid var(--border)', borderRadius: '8px', padding: '12px', marginBottom: '16px' }}>
        <legend style={{ fontWeight: 600, padding: '0 8px' }}>Conditions</legend>
        {conditions.map((c, i) => (
          <ConditionRow
            key={i}
            condition={c}
            onChange={(updated) => updateCondition(i, updated)}
            onRemove={() => setConditions(conditions.filter((_, idx) => idx !== i))}
          />
        ))}
        <button
          type="button"
          className="btn btn--text"
          onClick={() => setConditions([...conditions, { field: 'from', operator: 'contains', value: '' }])}
        >
          <Plus size={14} /> Add condition
        </button>
      </fieldset>

      <fieldset style={{ border: '1px solid var(--border)', borderRadius: '8px', padding: '12px', marginBottom: '16px' }}>
        <legend style={{ fontWeight: 600, padding: '0 8px' }}>Actions</legend>
        {actions.map((a, i) => (
          <ActionRow
            key={i}
            action={a}
            onChange={(updated) => updateAction(i, updated)}
            onRemove={() => setActions(actions.filter((_, idx) => idx !== i))}
          />
        ))}
        <button
          type="button"
          className="btn btn--text"
          onClick={() => setActions([...actions, { action_type: 'move', target: '' }])}
        >
          <Plus size={14} /> Add action
        </button>
      </fieldset>

      <label style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '16px' }}>
        <input
          type="checkbox"
          checked={stopProcessing}
          onChange={(e) => setStopProcessing(e.target.checked)}
        />
        Stop processing further rules after this one matches
      </label>

      <div style={{ display: 'flex', gap: '8px' }}>
        <button type="submit" className="btn btn--primary">
          {rule ? 'Update Rule' : 'Create Rule'}
        </button>
        <button type="button" className="btn" onClick={onCancel}>Cancel</button>
      </div>
    </form>
  );
}

export function FilterManager() {
  const [editing, setEditing] = useState<SieveRule | 'new' | null>(null);
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);

  const { data: rules = [], isLoading } = useQuery({
    queryKey: ['filters'],
    queryFn: listFilters,
  });

  const createMutation = useMutation({
    mutationFn: createFilter,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['filters'] });
      setEditing(null);
    },
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: CreateFilterRequest }) => updateFilter(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['filters'] });
      setEditing(null);
    },
  });

  const deleteMutation = useMutation({
    mutationFn: deleteFilter,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['filters'] }),
  });

  const toggleMutation = useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) => updateFilter(id, { enabled }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['filters'] }),
  });

  const reorderMutation = useMutation({
    mutationFn: reorderFilters,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['filters'] }),
  });

  const handleSave = (data: CreateFilterRequest) => {
    if (editing && editing !== 'new') {
      updateMutation.mutate({ id: (editing as SieveRule).id, data });
    } else {
      createMutation.mutate(data);
    }
  };

  // Added: move rule up/down in priority
  const moveRule = (index: number, direction: 'up' | 'down') => {
    const sorted = [...rules].sort((a, b) => a.priority - b.priority);
    const newIdx = direction === 'up' ? index - 1 : index + 1;
    if (newIdx < 0 || newIdx >= sorted.length) return;
    [sorted[index], sorted[newIdx]] = [sorted[newIdx], sorted[index]];
    reorderMutation.mutate(sorted.map((r) => r.id));
  };

  if (editing) {
    return (
      <div className="settings-panel">
        <button className="btn btn--text" onClick={() => setEditing(null)} style={{ marginBottom: '16px' }}>
          <ArrowLeft size={16} /> Back to filters
        </button>
        <h2>{editing === 'new' ? 'New Filter Rule' : `Edit: ${(editing as SieveRule).name}`}</h2>
        <FilterEditor
          rule={editing === 'new' ? undefined : (editing as SieveRule)}
          onSave={handleSave}
          onCancel={() => setEditing(null)}
        />
      </div>
    );
  }

  if (isLoading) return <LoadingSkeleton rows={5} />;

  const sortedRules = [...rules].sort((a, b) => a.priority - b.priority);

  return (
    <div className="settings-panel">
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
        <button className="btn btn--text" onClick={() => setViewMode('list')}>
          <ArrowLeft size={16} /> Back
        </button>
        <h2>Email Filters</h2>
        <button className="btn btn--primary" onClick={() => setEditing('new')}>
          <Plus size={16} /> New Filter
        </button>
      </div>

      {sortedRules.length === 0 ? (
        <p style={{ textAlign: 'center', color: 'var(--text-secondary)', padding: '40px 0' }}>
          No filter rules yet. Create one to automatically organize your email.
        </p>
      ) : (
        <div className="filter-list">
          {sortedRules.map((rule, index) => (
            <div
              key={rule.id}
              className="filter-item"
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: '12px',
                padding: '12px',
                borderBottom: '1px solid var(--border)',
                opacity: rule.enabled ? 1 : 0.5,
              }}
            >
              <div style={{ display: 'flex', flexDirection: 'column', gap: '2px' }}>
                <button
                  className="btn btn--icon"
                  onClick={() => moveRule(index, 'up')}
                  disabled={index === 0}
                  title="Move up"
                >
                  <ArrowUp size={12} />
                </button>
                <button
                  className="btn btn--icon"
                  onClick={() => moveRule(index, 'down')}
                  disabled={index === sortedRules.length - 1}
                  title="Move down"
                >
                  <ArrowDown size={12} />
                </button>
              </div>
              <div style={{ flex: 1 }}>
                <strong>{rule.name}</strong>
                <div style={{ fontSize: '0.85em', color: 'var(--text-secondary)' }}>
                  {rule.conditions.length} condition{rule.conditions.length !== 1 ? 's' : ''} ({rule.match_mode}) → {rule.actions.length} action{rule.actions.length !== 1 ? 's' : ''}
                  {rule.stop_processing && ' • stops'}
                </div>
              </div>
              <button
                className="btn btn--icon"
                onClick={() => toggleMutation.mutate({ id: rule.id, enabled: !rule.enabled })}
                title={rule.enabled ? 'Disable' : 'Enable'}
              >
                <Power size={16} color={rule.enabled ? 'var(--success)' : 'var(--text-secondary)'} />
              </button>
              <button className="btn btn--icon" onClick={() => setEditing(rule)} title="Edit">
                <Edit2 size={16} />
              </button>
              <button
                className="btn btn--icon"
                onClick={() => { if (confirm(`Delete filter "${rule.name}"?`)) deleteMutation.mutate(rule.id); }}
                title="Delete"
              >
                <Trash2 size={16} />
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
