// Added: Natural language search component for TMAIL-135
// PURPOSE: Provides an AI-powered search bar that parses natural language queries into structured email search
// EXTERNAL: Uses TanStack Query for search mutation, nlp-search API for AI parsing

import React, { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Search, Sparkles, Clock, Trash2, Mail, X } from 'lucide-react';
import { nlpSearch, listNlpHistory, clearNlpHistory } from '../../api/nlp-search';
import type { NlpSearchResult, NlpSearchHistoryEntry, ParsedSearchParams } from '../../api/nlp-search';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

// Added: Render parsed search parameters as readable badges
function ParsedParamsBadges({ params }: { params: ParsedSearchParams }) {
  const badges: { label: string; value: string }[] = [];
  if (params.from) badges.push({ label: 'From', value: params.from });
  if (params.to) badges.push({ label: 'To', value: params.to });
  if (params.subject) badges.push({ label: 'Subject', value: params.subject });
  if (params.folder) badges.push({ label: 'Folder', value: params.folder });
  if (params.date_from) badges.push({ label: 'After', value: params.date_from });
  if (params.date_to) badges.push({ label: 'Before', value: params.date_to });
  if (params.has_attachment) badges.push({ label: 'Has', value: 'Attachment' });
  params.keywords.forEach((kw) => badges.push({ label: 'Keyword', value: kw }));

  if (badges.length === 0) return null;

  return (
    <div style={{ display: 'flex', flexWrap: 'wrap', gap: '4px', marginTop: '8px' }} data-testid="parsed-params">
      {badges.map((badge, index) => (
        <span
          key={index}
          style={{
            fontSize: '11px',
            padding: '2px 8px',
            borderRadius: '10px',
            background: 'var(--color-bg-secondary)',
            border: '1px solid var(--color-border)',
            color: 'var(--color-text-secondary)',
          }}
        >
          <strong>{badge.label}:</strong> {badge.value}
        </span>
      ))}
    </div>
  );
}

// Added: Highlight matching keywords in text
function highlightKeywords(text: string, keywords: string[]): React.ReactNode {
  if (!keywords.length || !text) return text;

  // NOTE: Escape regex special characters in keywords
  const escaped = keywords.map((kw) => kw.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'));
  const pattern = new RegExp(`(${escaped.join('|')})`, 'gi');
  const parts = text.split(pattern);

  return parts.map((part, index) => {
    const isMatch = keywords.some((kw) => kw.toLowerCase() === part.toLowerCase());
    if (isMatch) {
      return (
        <mark key={index} style={{ background: '#fef08a', padding: '0 2px', borderRadius: '2px' }}>
          {part}
        </mark>
      );
    }
    return part;
  });
}

export function NlpSearch() {
  const queryClient = useQueryClient();
  const [queryText, setQueryText] = useState('');
  const [searchResult, setSearchResult] = useState<NlpSearchResult | null>(null);
  const [showHistory, setShowHistory] = useState(false);

  // Added: Search mutation for executing NLP queries
  const searchMut = useMutation({
    mutationFn: nlpSearch,
    onSuccess: (result) => {
      setSearchResult(result);
      setShowHistory(false);
      // NOTE: Invalidate history so it refreshes next time it's shown
      queryClient.invalidateQueries({ queryKey: ['nlp-search-history'] });
    },
  });

  // Added: Fetch search history
  const { data: history, isLoading: historyLoading } = useQuery({
    queryKey: ['nlp-search-history'],
    queryFn: listNlpHistory,
    enabled: showHistory,
  });

  // Added: Clear all history
  const clearMut = useMutation({
    mutationFn: clearNlpHistory,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['nlp-search-history'] });
    },
  });

  const handleSearch = (e: React.FormEvent) => {
    e.preventDefault();
    if (!queryText.trim()) return;
    searchMut.mutate(queryText.trim());
  };

  // Added: Re-run a previous search from history
  const handleHistoryClick = (entry: NlpSearchHistoryEntry) => {
    setQueryText(entry.query_text);
    searchMut.mutate(entry.query_text);
  };

  return (
    <div className="nlp-search" style={{ padding: '16px', maxWidth: '900px' }}>
      {/* Added: Search bar */}
      <form onSubmit={handleSearch} style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
        <div
          style={{
            flex: 1,
            display: 'flex',
            alignItems: 'center',
            gap: '8px',
            border: '1px solid var(--color-border)',
            borderRadius: '8px',
            padding: '8px 12px',
            background: 'var(--color-bg-secondary)',
          }}
        >
          <Sparkles size={16} style={{ color: 'var(--color-text-secondary)', flexShrink: 0 }} />
          <input
            value={queryText}
            onChange={(e) => setQueryText(e.target.value)}
            placeholder="Search with natural language, e.g. 'emails from John last week with attachments'"
            style={{
              flex: 1,
              border: 'none',
              background: 'transparent',
              outline: 'none',
              fontSize: '14px',
            }}
          />
          {queryText && (
            <button
              type="button"
              className="btn btn--icon"
              onClick={() => { setQueryText(''); setSearchResult(null); }}
              title="Clear"
              style={{ padding: '2px' }}
            >
              <X size={14} />
            </button>
          )}
        </div>
        <button
          type="submit"
          className="btn btn--primary"
          disabled={!queryText.trim() || searchMut.isPending}
        >
          <Search size={16} /> {searchMut.isPending ? 'Searching...' : 'Search'}
        </button>
        <button
          type="button"
          className="btn"
          onClick={() => setShowHistory(!showHistory)}
          title="Search History"
        >
          <Clock size={16} />
        </button>
      </form>

      {/* Added: Loading state during search */}
      {searchMut.isPending && (
        <div style={{ marginTop: '16px' }}>
          <LoadingSkeleton rows={4} />
        </div>
      )}

      {/* Added: Parsed intent display */}
      {searchResult && !searchMut.isPending && (
        <div style={{ marginTop: '16px' }}>
          <ParsedParamsBadges params={searchResult.parsed_params} />

          {/* Added: Results count */}
          <div
            style={{
              marginTop: '12px',
              fontSize: '13px',
              color: 'var(--color-text-secondary)',
              borderBottom: '1px solid var(--color-border)',
              paddingBottom: '8px',
            }}
          >
            {searchResult.result_count} result{searchResult.result_count !== 1 ? 's' : ''} for &ldquo;{searchResult.query}&rdquo;
          </div>

          {/* Added: Empty results state */}
          {searchResult.results.length === 0 && (
            <p style={{ color: 'var(--color-text-secondary)', textAlign: 'center', padding: '24px' }}>
              No emails matched your search. Try rephrasing your query.
            </p>
          )}

          {/* Added: Results list */}
          {searchResult.results.map((item, index) => (
            <div
              key={`${item.folder}-${item.uid}`}
              style={{
                padding: '10px 12px',
                borderBottom: '1px solid var(--color-border)',
                display: 'flex',
                alignItems: 'center',
                gap: '10px',
                cursor: 'pointer',
              }}
              data-testid={`search-result-${index}`}
            >
              <Mail size={16} style={{ color: 'var(--color-text-secondary)', flexShrink: 0 }} />
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ fontSize: '14px', fontWeight: 500 }}>
                  {highlightKeywords(item.subject || '(No Subject)', searchResult.parsed_params.keywords)}
                </div>
                <div style={{ fontSize: '12px', color: 'var(--color-text-secondary)', marginTop: '2px' }}>
                  {item.from && (
                    <span>{highlightKeywords(item.from, searchResult.parsed_params.keywords)}</span>
                  )}
                  {item.date && (
                    <span style={{ marginLeft: '8px' }}>
                      {new Date(item.date).toLocaleDateString()}
                    </span>
                  )}
                  <span
                    style={{
                      marginLeft: '8px',
                      fontSize: '11px',
                      padding: '0 4px',
                      borderRadius: '4px',
                      background: 'var(--color-bg-secondary)',
                    }}
                  >
                    {item.folder}
                  </span>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Added: Search history panel */}
      {showHistory && (
        <div
          style={{
            marginTop: '16px',
            border: '1px solid var(--color-border)',
            borderRadius: '8px',
            padding: '12px',
          }}
        >
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
            <h3 style={{ margin: 0, fontSize: '14px' }}>Search History</h3>
            {history && history.length > 0 && (
              <button
                className="btn btn--danger"
                onClick={() => clearMut.mutate()}
                disabled={clearMut.isPending}
                style={{ fontSize: '12px' }}
              >
                <Trash2 size={12} /> Clear All
              </button>
            )}
          </div>

          {historyLoading && <LoadingSkeleton rows={3} />}

          {!historyLoading && (!history || history.length === 0) && (
            <p style={{ color: 'var(--color-text-secondary)', textAlign: 'center', padding: '16px', fontSize: '12px' }}>
              No search history yet.
            </p>
          )}

          {history?.map((entry: NlpSearchHistoryEntry) => (
            <div
              key={entry.id}
              style={{
                padding: '8px',
                borderBottom: '1px solid var(--color-border)',
                cursor: 'pointer',
                fontSize: '13px',
              }}
              onClick={() => handleHistoryClick(entry)}
              data-testid={`history-${entry.id}`}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <span>&ldquo;{entry.query_text}&rdquo;</span>
                <span style={{ fontSize: '11px', color: 'var(--color-text-secondary)' }}>
                  {entry.result_count} result{entry.result_count !== 1 ? 's' : ''} &middot;{' '}
                  {new Date(entry.created_at).toLocaleDateString()}
                </span>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
