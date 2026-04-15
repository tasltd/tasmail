// Added: Archive management UI for Piler email archiving integration (TMAIL-107)
// PURPOSE: Allows admins to manage archive policies and config, users to search archived emails
// EXTERNAL: Uses TanStack Query for data fetching, Zustand for view state

import React from 'react';
import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, Trash2, ArrowLeft, ToggleLeft, ToggleRight, HardDrive, Search, Settings, Clock } from 'lucide-react';
import {
  listArchivePolicies,
  createArchivePolicy,
  updateArchivePolicy,
  deleteArchivePolicy,
  getArchiveConfig,
  updateArchiveConfig,
  searchArchive,
  getArchiveSearchHistory,
} from '../../api/archive';
import type {
  ArchivePolicy,
  ArchiveSearchResult,
  ArchiveSearchHistoryEntry,
} from '../../api/archive';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

// Added: Archive search tab content
function ArchiveSearchPanel() {
  const [searchQuery, setSearchQuery] = useState('');
  const [dateFrom, setDateFrom] = useState('');
  const [dateTo, setDateTo] = useState('');
  const [searchResults, setSearchResults] = useState<ArchiveSearchResult[] | null>(null);

  const { data: history } = useQuery({
    queryKey: ['archive-search-history'],
    queryFn: getArchiveSearchHistory,
  });

  const searchMut = useMutation({
    mutationFn: searchArchive,
    onSuccess: (results) => setSearchResults(results),
  });

  const handleSearch = (e: React.FormEvent) => {
    e.preventDefault();
    searchMut.mutate({
      query: searchQuery,
      date_from: dateFrom || undefined,
      date_to: dateTo || undefined,
    });
  };

  return (
    <div data-testid="archive-search-panel" style={{ marginTop: '16px' }}>
      <form onSubmit={handleSearch} style={{ padding: '16px', border: '1px solid var(--color-border)', borderRadius: '8px' }}>
        <h3 style={{ marginBottom: '12px', display: 'flex', alignItems: 'center', gap: '8px' }}>
          <Search size={18} /> Search Archived Emails
        </h3>
        <div className="composer__field">
          <label>Query</label>
          <input
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search archived emails..."
            required
          />
        </div>
        <div style={{ display: 'flex', gap: '12px', flexWrap: 'wrap', marginBottom: '12px' }}>
          <div className="composer__field" style={{ flex: '1 1 180px' }}>
            <label>From Date</label>
            <input
              type="date"
              value={dateFrom}
              onChange={(e) => setDateFrom(e.target.value)}
            />
          </div>
          <div className="composer__field" style={{ flex: '1 1 180px' }}>
            <label>To Date</label>
            <input
              type="date"
              value={dateTo}
              onChange={(e) => setDateTo(e.target.value)}
            />
          </div>
        </div>
        <div className="composer__actions">
          <button type="submit" className="btn btn--primary" disabled={searchMut.isPending || !searchQuery}>
            {searchMut.isPending ? 'Searching...' : 'Search Archive'}
          </button>
        </div>
      </form>

      {/* Added: Display search results */}
      {searchResults !== null && (
        <div style={{ marginTop: '16px' }}>
          {searchResults.length === 0 ? (
            <p style={{ color: 'var(--color-text-secondary)', textAlign: 'center', padding: '16px' }}>
              No archived emails found for this query.
            </p>
          ) : (
            <table style={{ width: '100%', fontSize: '13px', borderCollapse: 'collapse' }}>
              <thead>
                <tr style={{ borderBottom: '2px solid var(--color-border)' }}>
                  <th style={{ textAlign: 'left', padding: '6px 8px' }}>Subject</th>
                  <th style={{ textAlign: 'left', padding: '6px 8px' }}>Sender</th>
                  <th style={{ textAlign: 'left', padding: '6px 8px' }}>Date</th>
                  <th style={{ textAlign: 'left', padding: '6px 8px' }}>Size</th>
                </tr>
              </thead>
              <tbody>
                {searchResults.map((result) => (
                  <tr key={result.id} style={{ borderBottom: '1px solid var(--color-border)' }}>
                    <td style={{ padding: '6px 8px' }}>{result.subject}</td>
                    <td style={{ padding: '6px 8px' }}>{result.sender}</td>
                    <td style={{ padding: '6px 8px' }}>{new Date(result.date).toLocaleDateString()}</td>
                    <td style={{ padding: '6px 8px' }}>{Math.round(result.size / 1024)} KB</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      )}

      {/* Added: Recent search history */}
      {history && history.length > 0 && (
        <div style={{ marginTop: '24px' }}>
          <h4 style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '8px' }}>
            <Clock size={16} /> Recent Searches
          </h4>
          <div data-testid="search-history">
            {history.map((entry: ArchiveSearchHistoryEntry) => (
              <div
                key={entry.id}
                style={{
                  padding: '8px 12px',
                  borderBottom: '1px solid var(--color-border)',
                  fontSize: '13px',
                  display: 'flex',
                  justifyContent: 'space-between',
                }}
              >
                <span>{entry.query}</span>
                <span style={{ color: 'var(--color-text-secondary)' }}>
                  {entry.result_count !== null ? `${entry.result_count} results` : 'pending'} &middot;{' '}
                  {new Date(entry.searched_at).toLocaleString()}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

// Added: Archive config tab content
function ArchiveConfigPanel() {
  const queryClient = useQueryClient();
  const [formUrl, setFormUrl] = useState('');
  const [formApiKey, setFormApiKey] = useState('');
  const [formRetentionYears, setFormRetentionYears] = useState(7);
  const [formEnabled, setFormEnabled] = useState(false);
  const [initialized, setInitialized] = useState(false);

  const { data: config, isLoading } = useQuery({
    queryKey: ['archive-config'],
    queryFn: getArchiveConfig,
  });

  // Added: Initialize form with existing config data
  if (config && !initialized) {
    setFormUrl(config.piler_url ?? '');
    setFormRetentionYears(config.retention_years);
    setFormEnabled(config.enabled);
    setInitialized(true);
  }

  const updateMut = useMutation({
    mutationFn: updateArchiveConfig,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['archive-config'] }),
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    updateMut.mutate({
      piler_url: formUrl || undefined,
      piler_api_key: formApiKey || undefined,
      retention_years: formRetentionYears,
      enabled: formEnabled,
    });
  };

  if (isLoading) return <LoadingSkeleton rows={4} />;

  return (
    <div
      data-testid="archive-config-panel"
      style={{ marginTop: '16px', padding: '16px', border: '1px solid var(--color-border)', borderRadius: '8px' }}
    >
      <h3 style={{ marginBottom: '12px', display: 'flex', alignItems: 'center', gap: '8px' }}>
        <Settings size={18} /> Piler Server Configuration
      </h3>
      <form onSubmit={handleSubmit}>
        <div className="composer__field">
          <label>Piler URL</label>
          <input
            value={formUrl}
            onChange={(e) => setFormUrl(e.target.value)}
            placeholder="https://piler.example.com"
          />
        </div>
        <div className="composer__field">
          <label>API Key</label>
          <input
            type="password"
            value={formApiKey}
            onChange={(e) => setFormApiKey(e.target.value)}
            placeholder="Enter API key (leave blank to keep existing)"
          />
        </div>
        <div className="composer__field">
          <label>Retention Years</label>
          <input
            type="number"
            min={1}
            value={formRetentionYears}
            onChange={(e) => setFormRetentionYears(Number(e.target.value))}
          />
        </div>
        <div style={{ marginBottom: '12px' }}>
          <label style={{ display: 'flex', gap: '8px', alignItems: 'center', fontSize: '13px' }}>
            <input
              type="checkbox"
              checked={formEnabled}
              onChange={(e) => setFormEnabled(e.target.checked)}
            />
            Enable archiving
          </label>
        </div>
        <div className="composer__actions">
          <button type="submit" className="btn btn--primary" disabled={updateMut.isPending}>
            {updateMut.isPending ? 'Saving...' : 'Save Configuration'}
          </button>
        </div>
      </form>
      {/* Added: Show current status */}
      {config && (
        <div style={{ marginTop: '12px', fontSize: '12px', color: 'var(--color-text-secondary)' }}>
          Status: {config.enabled ? 'Enabled' : 'Disabled'}
          {config.piler_url && <> &middot; Connected to {config.piler_url}</>}
        </div>
      )}
    </div>
  );
}

export function ArchiveManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);
  const [isCreating, setIsCreating] = useState(false);
  const [activeTab, setActiveTab] = useState<'policies' | 'config' | 'search'>('policies');

  // Added: Form state for creating new archive policies
  const [formName, setFormName] = useState('');
  const [formDescription, setFormDescription] = useState('');
  const [formDomains, setFormDomains] = useState('*');
  const [formFolders, setFormFolders] = useState('INBOX,Sent');
  const [formArchiveDays, setFormArchiveDays] = useState(90);
  const [formDeleteOriginal, setFormDeleteOriginal] = useState(false);

  const { data: policies, isLoading } = useQuery({
    queryKey: ['archive-policies'],
    queryFn: listArchivePolicies,
  });

  const createMut = useMutation({
    mutationFn: createArchivePolicy,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['archive-policies'] });
      setIsCreating(false);
      // NOTE: Reset form for next use
      setFormName('');
      setFormDescription('');
      setFormDomains('*');
      setFormFolders('INBOX,Sent');
      setFormArchiveDays(90);
      setFormDeleteOriginal(false);
    },
  });

  const toggleMut = useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) =>
      updateArchivePolicy(id, { enabled }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['archive-policies'] }),
  });

  const deleteMut = useMutation({
    mutationFn: deleteArchivePolicy,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['archive-policies'] }),
  });

  const handleCreate = (e: React.FormEvent) => {
    e.preventDefault();
    // Added: Parse comma-separated domains and folders into match_criteria
    const match_criteria: Record<string, string[]> = {};
    if (formDomains.trim()) {
      match_criteria.domains = formDomains.split(',').map((d) => d.trim());
    }
    if (formFolders.trim()) {
      match_criteria.folders = formFolders.split(',').map((f) => f.trim());
    }

    createMut.mutate({
      name: formName,
      description: formDescription || undefined,
      match_criteria,
      archive_after_days: formArchiveDays,
      delete_original: formDeleteOriginal,
    });
  };

  if (isLoading) return <LoadingSkeleton rows={4} />;

  return (
    <div className="archive-manager" style={{ padding: '16px', maxWidth: '1000px' }}>
      <div className="message-view__toolbar">
        <button className="btn btn--icon" onClick={() => setViewMode('list')} title="Back">
          <ArrowLeft size={20} />
        </button>
        <h2 style={{ flex: 1, fontSize: '18px' }}>Email Archive</h2>
        {activeTab === 'policies' && (
          <button className="btn btn--primary" onClick={() => setIsCreating(true)}>
            <Plus size={16} /> Add Policy
          </button>
        )}
      </div>

      {/* Added: Tab navigation for policies, config, and search */}
      <div style={{ display: 'flex', gap: '4px', marginTop: '12px', borderBottom: '1px solid var(--color-border)' }}>
        {(['policies', 'config', 'search'] as const).map((tab) => (
          <button
            key={tab}
            onClick={() => setActiveTab(tab)}
            style={{
              padding: '8px 16px',
              background: activeTab === tab ? 'var(--color-primary, #3b82f6)' : 'transparent',
              color: activeTab === tab ? 'white' : 'inherit',
              border: 'none',
              borderRadius: '4px 4px 0 0',
              cursor: 'pointer',
              textTransform: 'capitalize',
            }}
          >
            {tab.charAt(0).toUpperCase() + tab.slice(1)}
          </button>
        ))}
      </div>

      {/* Added: Policies tab content */}
      {activeTab === 'policies' && (
        <>
          {/* Added: Create archive policy form */}
          {isCreating && (
            <div
              style={{
                marginTop: '16px',
                padding: '16px',
                border: '1px solid var(--color-border)',
                borderRadius: '8px',
              }}
            >
              <h3 style={{ marginBottom: '12px' }}>New Archive Policy</h3>
              <form onSubmit={handleCreate}>
                <div className="composer__field">
                  <label>Name</label>
                  <input
                    value={formName}
                    onChange={(e) => setFormName(e.target.value)}
                    placeholder="Archive All INBOX"
                    required
                  />
                </div>
                <div className="composer__field">
                  <label>Description</label>
                  <input
                    value={formDescription}
                    onChange={(e) => setFormDescription(e.target.value)}
                    placeholder="Optional description"
                  />
                </div>
                <div style={{ display: 'flex', gap: '12px', flexWrap: 'wrap', marginBottom: '12px' }}>
                  <div className="composer__field" style={{ flex: '1 1 200px' }}>
                    <label>Domains (comma-separated)</label>
                    <input
                      value={formDomains}
                      onChange={(e) => setFormDomains(e.target.value)}
                      placeholder="*, example.com"
                    />
                  </div>
                  <div className="composer__field" style={{ flex: '1 1 200px' }}>
                    <label>Folders (comma-separated)</label>
                    <input
                      value={formFolders}
                      onChange={(e) => setFormFolders(e.target.value)}
                      placeholder="INBOX, Sent"
                    />
                  </div>
                </div>
                <div style={{ display: 'flex', gap: '12px', flexWrap: 'wrap', marginBottom: '12px' }}>
                  <div className="composer__field" style={{ flex: '1 1 150px' }}>
                    <label>Archive After (days)</label>
                    <input
                      type="number"
                      min={1}
                      value={formArchiveDays}
                      onChange={(e) => setFormArchiveDays(Number(e.target.value))}
                    />
                  </div>
                </div>
                <div style={{ marginBottom: '12px' }}>
                  <label style={{ display: 'flex', gap: '8px', alignItems: 'center', fontSize: '13px' }}>
                    <input
                      type="checkbox"
                      checked={formDeleteOriginal}
                      onChange={(e) => setFormDeleteOriginal(e.target.checked)}
                    />
                    Delete original after archiving
                  </label>
                </div>
                <div className="composer__actions">
                  <button type="submit" className="btn btn--primary" disabled={!formName}>
                    Create
                  </button>
                  <button type="button" className="btn" onClick={() => setIsCreating(false)}>
                    Cancel
                  </button>
                </div>
              </form>
            </div>
          )}

          {/* Added: Policies list */}
          <div style={{ marginTop: '16px' }}>
            {(!policies || policies.length === 0) && !isCreating && (
              <p style={{ color: 'var(--color-text-secondary)', textAlign: 'center', padding: '24px' }}>
                No archive policies configured. Add one to start archiving emails with Piler.
              </p>
            )}
            {policies?.map((policy: ArchivePolicy) => (
              <div
                key={policy.id}
                style={{
                  padding: '12px',
                  borderBottom: '1px solid var(--color-border)',
                }}
              >
                <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                  <HardDrive size={18} style={{ color: 'var(--color-text-secondary)' }} />
                  <div style={{ flex: 1 }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                      <strong style={{ fontSize: '14px' }}>{policy.name}</strong>
                      <span
                        style={{
                          fontSize: '11px',
                          padding: '1px 6px',
                          borderRadius: '10px',
                          background: policy.enabled ? 'green' : 'gray',
                          color: 'white',
                        }}
                      >
                        {policy.enabled ? 'Active' : 'Inactive'}
                      </span>
                      <span style={{ fontSize: '12px', color: 'var(--color-text-secondary)' }}>
                        {policy.archive_after_days} days
                      </span>
                      {policy.delete_original && (
                        <span
                          style={{
                            fontSize: '11px',
                            padding: '1px 6px',
                            borderRadius: '10px',
                            background: '#f97316',
                            color: 'white',
                          }}
                        >
                          Deletes Original
                        </span>
                      )}
                    </div>
                    <div style={{ fontSize: '12px', color: 'var(--color-text-secondary)', marginTop: '2px' }}>
                      {policy.description && <>{policy.description} &middot; </>}
                      Criteria: {JSON.stringify(policy.match_criteria)}
                    </div>
                  </div>
                  {/* Added: Active/inactive toggle */}
                  <button
                    className="btn btn--icon"
                    onClick={() => toggleMut.mutate({ id: policy.id, enabled: !policy.enabled })}
                    title={policy.enabled ? 'Disable' : 'Enable'}
                    data-testid={`toggle-${policy.id}`}
                  >
                    {policy.enabled ? <ToggleRight size={20} /> : <ToggleLeft size={20} />}
                  </button>
                  {/* Added: Delete button */}
                  <button
                    className="btn btn--icon btn--danger"
                    onClick={() => deleteMut.mutate(policy.id)}
                    title="Delete"
                  >
                    <Trash2 size={16} />
                  </button>
                </div>
              </div>
            ))}
          </div>
        </>
      )}

      {/* Added: Config tab content */}
      {activeTab === 'config' && <ArchiveConfigPanel />}

      {/* Added: Search tab content */}
      {activeTab === 'search' && <ArchiveSearchPanel />}
    </div>
  );
}
