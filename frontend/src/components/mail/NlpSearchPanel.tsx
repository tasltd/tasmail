// Added: NLP search panel component for TMAIL-135
// PURPOSE: UI for AI-powered natural language email search with parsed parameter display and history
// EXTERNAL: Uses nlp-search API for search execution and history management

import { useState, useCallback } from 'react';
import { type FormEvent } from 'react';
import { BrainCircuit, Search, Loader2, History, Trash2 } from 'lucide-react';
import { useMailStore } from '../../stores/mailStore';
import { nlpSearch, listNlpHistory, clearNlpHistory } from '../../api/nlp-search';
import type { NlpSearchResult, NlpSearchHistoryEntry, ParsedSearchParams } from '../../api/nlp-search';

export function NlpSearchPanel() {
  const [queryInput, setQueryInput] = useState('');
  const [searchResult, setSearchResult] = useState<NlpSearchResult | null>(null);
  const [isSearching, setIsSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [hasSearched, setHasSearched] = useState(false);

  // Added: History state
  const [history, setHistory] = useState<NlpSearchHistoryEntry[]>([]);
  const [showHistory, setShowHistory] = useState(false);
  const [isLoadingHistory, setIsLoadingHistory] = useState(false);

  const setSelectedUid = useMailStore((s) => s.setSelectedUid);
  const setSelectedFolder = useMailStore((s) => s.setSelectedFolder);

  // PURPOSE: Execute NLP search with the current query input
  const handleSearch = useCallback(
    async (e: FormEvent) => {
      e.preventDefault();
      if (queryInput.trim().length < 3) return;

      setIsSearching(true);
      setSearchError(null);
      setHasSearched(true);
      setShowHistory(false);

      try {
        const result = await nlpSearch(queryInput.trim());
        setSearchResult(result);
      } catch (err) {
        setSearchError(
          err instanceof Error ? err.message : 'NLP search failed. Ensure you have an active AI configuration.',
        );
        setSearchResult(null);
      } finally {
        setIsSearching(false);
      }
    },
    [queryInput],
  );

  // PURPOSE: Load search history from the backend
  const handleLoadHistory = useCallback(async () => {
    if (showHistory) {
      setShowHistory(false);
      return;
    }
    setIsLoadingHistory(true);
    try {
      const entries = await listNlpHistory();
      setHistory(entries);
      setShowHistory(true);
    } catch {
      // NOTE: History is informational; silently ignore errors
      setHistory([]);
      setShowHistory(true);
    } finally {
      setIsLoadingHistory(false);
    }
  }, [showHistory]);

  // PURPOSE: Clear all search history
  const handleClearHistory = useCallback(async () => {
    try {
      await clearNlpHistory();
      setHistory([]);
    } catch {
      // NOTE: Silently ignore clear errors
    }
  }, []);

  // PURPOSE: Re-run a query from history
  const handleHistoryClick = useCallback((queryText: string) => {
    setQueryInput(queryText);
    setShowHistory(false);
  }, []);

  // PURPOSE: Navigate to a specific email when a result is clicked
  const handleResultClick = useCallback(
    (folder: string, uid: number) => {
      setSelectedFolder(folder);
      setSelectedUid(uid);
    },
    [setSelectedFolder, setSelectedUid],
  );

  // Added: Format parsed parameters as a readable summary
  const formatParsedParams = (params: ParsedSearchParams): string => {
    const parts: string[] = [];
    if (params.from) parts.push(`From: ${params.from}`);
    if (params.to) parts.push(`To: ${params.to}`);
    if (params.subject) parts.push(`Subject: ${params.subject}`);
    if (params.keywords?.length > 0) parts.push(`Keywords: ${params.keywords.join(', ')}`);
    if (params.date_from) parts.push(`After: ${params.date_from}`);
    if (params.date_to) parts.push(`Before: ${params.date_to}`);
    if (params.folder) parts.push(`Folder: ${params.folder}`);
    if (params.has_attachment) parts.push('Has attachments');
    return parts.join(' | ');
  };

  return (
    <div className="nlp-search" data-testid="nlp-search-panel">
      {/* Added: Search input form */}
      <form className="nlp-search__form" onSubmit={handleSearch}>
        <div className="nlp-search__input-wrapper">
          <BrainCircuit size={16} />
          <input
            type="text"
            className="nlp-search__input"
            placeholder="Ask AI (e.g., 'emails from John about budget last week')..."
            value={queryInput}
            onChange={(e) => setQueryInput(e.target.value)}
            data-testid="nlp-search-input"
          />
        </div>
        <button
          type="submit"
          className="btn btn--primary nlp-search__btn"
          disabled={isSearching || queryInput.trim().length < 3}
          data-testid="nlp-search-btn"
        >
          {isSearching ? <Loader2 size={16} className="animate-spin" /> : <Search size={16} />}
          Ask AI
        </button>
        {/* Added: History toggle button */}
        <button
          type="button"
          className="btn btn--secondary nlp-search__history-btn"
          onClick={handleLoadHistory}
          disabled={isLoadingHistory}
          data-testid="nlp-search-history-btn"
        >
          <History size={14} />
          {isLoadingHistory ? 'Loading...' : 'History'}
        </button>
      </form>

      {/* Added: Search error display */}
      {searchError && (
        <div className="nlp-search__error" data-testid="nlp-search-error">
          {searchError}
        </div>
      )}

      {/* Added: Loading indicator */}
      {isSearching && (
        <div className="nlp-search__loading" data-testid="nlp-search-loading">
          <Loader2 size={20} className="animate-spin" />
          AI is parsing your query...
        </div>
      )}

      {/* Added: Parsed parameters display */}
      {searchResult && !isSearching && (
        <div className="nlp-search__parsed" data-testid="nlp-search-parsed">
          <div className="nlp-search__parsed-label">AI understood:</div>
          <div className="nlp-search__parsed-params" data-testid="nlp-search-parsed-params">
            {formatParsedParams(searchResult.parsed_params)}
          </div>
        </div>
      )}

      {/* Added: Empty results message */}
      {!isSearching && hasSearched && searchResult && searchResult.results.length === 0 && !searchError && (
        <div className="nlp-search__empty" data-testid="nlp-search-empty">
          No matching emails found. Try rephrasing your query.
        </div>
      )}

      {/* Added: Search results list */}
      {searchResult && searchResult.results.length > 0 && (
        <div className="nlp-search__results" data-testid="nlp-search-results">
          <div className="nlp-search__results-header">
            {searchResult.result_count} result{searchResult.result_count !== 1 ? 's' : ''} found
          </div>
          {searchResult.results.map((item) => (
            <div
              key={`${item.folder}-${item.uid}`}
              className="nlp-search__result-row"
              onClick={() => handleResultClick(item.folder, item.uid)}
              role="button"
              tabIndex={0}
              onKeyDown={(e) => e.key === 'Enter' && handleResultClick(item.folder, item.uid)}
              data-testid="nlp-search-result-item"
            >
              <div className="nlp-search__result-subject">
                {item.subject || '(no subject)'}
              </div>
              <div className="nlp-search__result-meta">
                <span className="nlp-search__result-from">{item.from || 'Unknown'}</span>
                <span className="nlp-search__result-folder">{item.folder}</span>
                {item.date && <span className="nlp-search__result-date">{item.date}</span>}
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Added: Search history dropdown */}
      {showHistory && (
        <div className="nlp-search__history" data-testid="nlp-search-history">
          <div className="nlp-search__history-header">
            <span>Recent searches</span>
            {history.length > 0 && (
              <button
                type="button"
                className="btn btn--icon nlp-search__clear-btn"
                onClick={handleClearHistory}
                title="Clear all history"
                data-testid="nlp-search-clear-btn"
              >
                <Trash2 size={14} />
              </button>
            )}
          </div>
          {history.length === 0 && (
            <div className="nlp-search__history-empty">No search history yet.</div>
          )}
          {history.map((entry) => (
            <div
              key={entry.id}
              className="nlp-search__history-item"
              onClick={() => handleHistoryClick(entry.query_text)}
              role="button"
              tabIndex={0}
              onKeyDown={(e) => e.key === 'Enter' && handleHistoryClick(entry.query_text)}
              data-testid="nlp-search-history-item"
            >
              <div className="nlp-search__history-query">{entry.query_text}</div>
              <div className="nlp-search__history-meta">
                {entry.result_count} results
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
