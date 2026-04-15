// Added: eDiscovery search management UI for compliance investigations (TMAIL-137)
// PURPOSE: Allows admins to create, execute, and export eDiscovery searches
// EXTERNAL: Uses TanStack Query for data fetching, Zustand for view state

import React from 'react';
import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, Trash2, ArrowLeft, Search, Play, Download, Eye } from 'lucide-react';
import {
  listEdiscoverySearches,
  createEdiscoverySearch,
  getEdiscoverySearch,
  deleteEdiscoverySearch,
  executeEdiscoverySearch,
  exportEdiscoveryResults,
} from '../../api/ediscovery';
import type { EdiscoverySearch } from '../../api/ediscovery';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

export function EdiscoveryManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);

  // Added: Form visibility and detail view state
  const [showCreateForm, setShowCreateForm] = useState(false);
  const [selectedSearchId, setSelectedSearchId] = useState<string | null>(null);

  // Added: Create form state
  const [searchName, setSearchName] = useState('');
  const [searchDescription, setSearchDescription] = useState('');
  const [searchQuery, setSearchQuery] = useState('');
  const [targetUsers, setTargetUsers] = useState('');
  const [dateFrom, setDateFrom] = useState('');
  const [dateTo, setDateTo] = useState('');
  const [includeAttachments, setIncludeAttachments] = useState(false);

  // Added: Fetch all eDiscovery searches
  const { data: searches, isLoading: searchesLoading } = useQuery({
    queryKey: ['ediscovery-searches'],
    queryFn: listEdiscoverySearches,
  });

  // Added: Fetch selected search detail with results
  const { data: searchDetail, isLoading: detailLoading } = useQuery({
    queryKey: ['ediscovery-search', selectedSearchId],
    queryFn: () => getEdiscoverySearch(selectedSearchId!),
    enabled: !!selectedSearchId,
  });

  // Added: Create search mutation
  const createMut = useMutation({
    mutationFn: createEdiscoverySearch,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['ediscovery-searches'] });
      setShowCreateForm(false);
      resetForm();
    },
  });

  // Added: Delete search mutation
  const deleteMut = useMutation({
    mutationFn: deleteEdiscoverySearch,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['ediscovery-searches'] });
      setSelectedSearchId(null);
    },
  });

  // Added: Execute search mutation
  const executeMut = useMutation({
    mutationFn: executeEdiscoverySearch,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['ediscovery-searches'] });
      if (selectedSearchId) {
        queryClient.invalidateQueries({ queryKey: ['ediscovery-search', selectedSearchId] });
      }
    },
  });

  // Added: Export results mutation
  const exportMut = useMutation({
    mutationFn: exportEdiscoveryResults,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['ediscovery-searches'] });
      if (selectedSearchId) {
        queryClient.invalidateQueries({ queryKey: ['ediscovery-search', selectedSearchId] });
      }
    },
  });

  // Added: Reset form fields helper
  const resetForm = () => {
    setSearchName('');
    setSearchDescription('');
    setSearchQuery('');
    setTargetUsers('');
    setDateFrom('');
    setDateTo('');
    setIncludeAttachments(false);
  };

  const handleCreate = (e: React.FormEvent) => {
    e.preventDefault();
    // Added: Parse comma-separated user IDs into array
    const userIds = targetUsers.trim()
      ? targetUsers.split(',').map((uid) => uid.trim()).filter(Boolean)
      : undefined;

    createMut.mutate({
      name: searchName,
      description: searchDescription || undefined,
      search_query: searchQuery,
      target_users: userIds,
      date_from: dateFrom ? new Date(dateFrom).toISOString() : undefined,
      date_to: dateTo ? new Date(dateTo).toISOString() : undefined,
      include_attachments: includeAttachments,
    });
  };

  // Added: Status badge color mapping
  const statusColor = (status: string): string => {
    switch (status) {
      case 'Pending': return '#6b7280';
      case 'Running': return '#3b82f6';
      case 'Completed': return '#10b981';
      case 'Failed': return '#ef4444';
      case 'Exported': return '#8b5cf6';
      default: return '#6b7280';
    }
  };

  if (searchesLoading) return <LoadingSkeleton rows={4} />;

  // Added: Detail view when a search is selected
  if (selectedSearchId && searchDetail) {
    return (
      <div className="ediscovery-manager" style={{ padding: '16px', maxWidth: '1000px' }}>
        <div className="message-view__toolbar">
          <button className="btn btn--icon" onClick={() => setSelectedSearchId(null)} title="Back to list">
            <ArrowLeft size={20} />
          </button>
          <h2 style={{ flex: 1, fontSize: '18px' }}>{searchDetail.name}</h2>
          <span
            style={{
              fontSize: '11px',
              padding: '2px 8px',
              borderRadius: '10px',
              background: statusColor(searchDetail.status),
              color: 'white',
            }}
          >
            {searchDetail.status}
          </span>
        </div>

        {/* Added: Search metadata */}
        <div style={{ marginTop: '16px', fontSize: '13px', color: 'var(--color-text-secondary)' }}>
          <p>Query: <strong>{searchDetail.search_query}</strong></p>
          {searchDetail.description && <p>Description: {searchDetail.description}</p>}
          {searchDetail.date_from && <p>From: {new Date(searchDetail.date_from).toLocaleDateString()}</p>}
          {searchDetail.date_to && <p>To: {new Date(searchDetail.date_to).toLocaleDateString()}</p>}
          <p>Include attachments: {searchDetail.include_attachments ? 'Yes' : 'No'}</p>
          <p>Results: {searchDetail.results_count ?? 0}</p>
        </div>

        {/* Added: Action buttons */}
        <div style={{ marginTop: '16px', display: 'flex', gap: '8px' }}>
          {searchDetail.status === 'Pending' && (
            <button
              className="btn btn--primary"
              onClick={() => executeMut.mutate(selectedSearchId)}
              disabled={executeMut.isPending}
            >
              <Play size={16} /> Execute
            </button>
          )}
          {searchDetail.status === 'Completed' && (
            <button
              className="btn btn--primary"
              onClick={() => exportMut.mutate(selectedSearchId)}
              disabled={exportMut.isPending}
            >
              <Download size={16} /> Export MBOX
            </button>
          )}
          <button
            className="btn btn--danger"
            onClick={() => deleteMut.mutate(selectedSearchId)}
          >
            <Trash2 size={16} /> Delete
          </button>
        </div>

        {/* Added: Results table */}
        {searchDetail.results && searchDetail.results.length > 0 && (
          <table style={{ width: '100%', marginTop: '24px', borderCollapse: 'collapse', fontSize: '13px' }}>
            <thead>
              <tr style={{ borderBottom: '2px solid var(--color-border)', textAlign: 'left' }}>
                <th style={{ padding: '8px' }}>Subject</th>
                <th style={{ padding: '8px' }}>From</th>
                <th style={{ padding: '8px' }}>Folder</th>
                <th style={{ padding: '8px' }}>Date</th>
                <th style={{ padding: '8px' }}>Score</th>
              </tr>
            </thead>
            <tbody>
              {searchDetail.results.map((result) => (
                <tr key={result.id} style={{ borderBottom: '1px solid var(--color-border)' }}>
                  <td style={{ padding: '8px' }}>{result.subject || '(no subject)'}</td>
                  <td style={{ padding: '8px' }}>{result.from_address || 'unknown'}</td>
                  <td style={{ padding: '8px' }}>{result.folder}</td>
                  <td style={{ padding: '8px' }}>
                    {result.date ? new Date(result.date).toLocaleDateString() : '-'}
                  </td>
                  <td style={{ padding: '8px' }}>
                    {result.relevance_score != null ? result.relevance_score.toFixed(2) : '-'}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}

        {searchDetail.results && searchDetail.results.length === 0 && (
          <p style={{ color: 'var(--color-text-secondary)', textAlign: 'center', padding: '24px', marginTop: '16px' }}>
            No results found for this search.
          </p>
        )}
      </div>
    );
  }

  // Added: Detail loading state
  if (selectedSearchId && detailLoading) {
    return <LoadingSkeleton rows={6} />;
  }

  return (
    <div className="ediscovery-manager" style={{ padding: '16px', maxWidth: '900px' }}>
      <div className="message-view__toolbar">
        <button className="btn btn--icon" onClick={() => setViewMode('list')} title="Back">
          <ArrowLeft size={20} />
        </button>
        <h2 style={{ flex: 1, fontSize: '18px' }}>eDiscovery</h2>
      </div>

      {/* Added: Search list section */}
      <section style={{ marginTop: '24px' }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '12px' }}>
          <h3 style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <Search size={18} />
            Searches
          </h3>
          <button
            className="btn btn--primary"
            onClick={() => setShowCreateForm(true)}
          >
            <Plus size={16} /> New Search
          </button>
        </div>

        {/* Added: Create search form */}
        {showCreateForm && (
          <div
            style={{
              padding: '16px',
              border: '1px solid var(--color-border)',
              borderRadius: '8px',
              marginBottom: '16px',
            }}
          >
            <h4 style={{ marginBottom: '12px' }}>New eDiscovery Search</h4>
            <form onSubmit={handleCreate}>
              <div className="composer__field">
                <label>Name</label>
                <input
                  value={searchName}
                  onChange={(e) => setSearchName(e.target.value)}
                  placeholder="e.g., Q1 Compliance Review"
                  required
                />
              </div>
              <div className="composer__field">
                <label>Description</label>
                <input
                  value={searchDescription}
                  onChange={(e) => setSearchDescription(e.target.value)}
                  placeholder="Optional description"
                />
              </div>
              <div className="composer__field">
                <label>Search Query</label>
                <input
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  placeholder="e.g., confidential contract"
                  required
                />
              </div>
              <div className="composer__field">
                <label>Target Users (comma-separated UUIDs)</label>
                <input
                  value={targetUsers}
                  onChange={(e) => setTargetUsers(e.target.value)}
                  placeholder="Leave empty to search all users"
                />
              </div>
              <div style={{ display: 'flex', gap: '12px' }}>
                <div className="composer__field" style={{ flex: 1 }}>
                  <label>Date From</label>
                  <input
                    type="date"
                    value={dateFrom}
                    onChange={(e) => setDateFrom(e.target.value)}
                  />
                </div>
                <div className="composer__field" style={{ flex: 1 }}>
                  <label>Date To</label>
                  <input
                    type="date"
                    value={dateTo}
                    onChange={(e) => setDateTo(e.target.value)}
                  />
                </div>
              </div>
              <div className="composer__field">
                <label style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  <input
                    type="checkbox"
                    checked={includeAttachments}
                    onChange={(e) => setIncludeAttachments(e.target.checked)}
                  />
                  Include attachments
                </label>
              </div>
              <div className="composer__actions">
                <button type="submit" className="btn btn--primary" disabled={!searchName || !searchQuery}>
                  Create
                </button>
                <button type="button" className="btn" onClick={() => { setShowCreateForm(false); resetForm(); }}>
                  Cancel
                </button>
              </div>
            </form>
          </div>
        )}

        {/* Added: Empty state */}
        {(!searches || searches.length === 0) && !showCreateForm && (
          <p style={{ color: 'var(--color-text-secondary)', textAlign: 'center', padding: '24px' }}>
            No eDiscovery searches yet. Create one to search across user mailboxes.
          </p>
        )}

        {/* Added: Search list */}
        {searches?.map((search: EdiscoverySearch) => (
          <div
            key={search.id}
            style={{
              padding: '12px',
              borderBottom: '1px solid var(--color-border)',
              display: 'flex',
              alignItems: 'center',
              gap: '12px',
            }}
          >
            <Search size={18} style={{ color: 'var(--color-text-secondary)' }} />
            <div style={{ flex: 1 }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                <strong style={{ fontSize: '14px' }}>{search.name}</strong>
                <span
                  style={{
                    fontSize: '11px',
                    padding: '1px 6px',
                    borderRadius: '10px',
                    background: statusColor(search.status),
                    color: 'white',
                  }}
                >
                  {search.status}
                </span>
              </div>
              <div style={{ fontSize: '12px', color: 'var(--color-text-secondary)', marginTop: '2px' }}>
                Query: &ldquo;{search.search_query}&rdquo;
                {search.results_count != null && <> &middot; {search.results_count} results</>}
                {' '}&middot; {new Date(search.created_at).toLocaleDateString()}
              </div>
            </div>
            {/* Added: View details button */}
            <button
              className="btn btn--icon"
              onClick={() => setSelectedSearchId(search.id)}
              title="View details"
              data-testid={`view-${search.id}`}
            >
              <Eye size={16} />
            </button>
            {/* Added: Quick execute for pending searches */}
            {search.status === 'Pending' && (
              <button
                className="btn btn--icon"
                onClick={() => executeMut.mutate(search.id)}
                title="Execute search"
                data-testid={`execute-${search.id}`}
              >
                <Play size={16} />
              </button>
            )}
            {/* Added: Quick export for completed searches */}
            {search.status === 'Completed' && (
              <button
                className="btn btn--icon"
                onClick={() => exportMut.mutate(search.id)}
                title="Export results"
                data-testid={`export-${search.id}`}
              >
                <Download size={16} />
              </button>
            )}
            <button
              className="btn btn--icon btn--danger"
              onClick={() => deleteMut.mutate(search.id)}
              title="Delete search"
            >
              <Trash2 size={16} />
            </button>
          </div>
        ))}
      </section>
    </div>
  );
}
