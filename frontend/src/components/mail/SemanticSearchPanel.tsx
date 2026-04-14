// Added: Semantic search panel component for TMAIL-106
// PURPOSE: UI for natural language email search using vector similarity via pgvector
// EXTERNAL: Uses semantic-search API for search, indexing, and stats

import { useState, useCallback } from 'react';
import { Search, Database, Loader2 } from 'lucide-react';
import { useMailStore } from '../../stores/mailStore';
import { semanticSearch, getIndexStats } from '../../api/semantic-search';
import type { SemanticSearchResult, IndexStatsResponse } from '../../api/semantic-search';

export function SemanticSearchPanel() {
  const [queryInput, setQueryInput] = useState('');
  const [searchResults, setSearchResults] = useState<SemanticSearchResult[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [hasSearched, setHasSearched] = useState(false);

  // Added: Index stats state
  const [indexStats, setIndexStats] = useState<IndexStatsResponse | null>(null);
  const [isLoadingStats, setIsLoadingStats] = useState(false);

  const setSelectedUid = useMailStore((s) => s.setSelectedUid);
  const setSelectedFolder = useMailStore((s) => s.setSelectedFolder);

  // PURPOSE: Execute semantic search with the current query input
  const handleSearch = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      if (queryInput.trim().length < 2) return;

      setIsSearching(true);
      setSearchError(null);
      setHasSearched(true);

      try {
        const results = await semanticSearch(queryInput.trim());
        setSearchResults(results);
      } catch (err) {
        setSearchError(
          err instanceof Error ? err.message : 'Semantic search failed. Ensure you have an active AI configuration.',
        );
        setSearchResults([]);
      } finally {
        setIsSearching(false);
      }
    },
    [queryInput],
  );

  // PURPOSE: Load index statistics on demand
  const handleLoadStats = useCallback(async () => {
    setIsLoadingStats(true);
    try {
      const stats = await getIndexStats();
      setIndexStats(stats);
    } catch {
      // NOTE: Stats are informational; silently ignore errors
      setIndexStats(null);
    } finally {
      setIsLoadingStats(false);
    }
  }, []);

  // PURPOSE: Navigate to a specific email when a result is clicked
  const handleResultClick = useCallback(
    (result: SemanticSearchResult) => {
      setSelectedFolder(result.folder);
      setSelectedUid(result.uid);
    },
    [setSelectedFolder, setSelectedUid],
  );

  // Added: Format similarity score as a percentage
  const formatScore = (score: number): string => {
    return `${Math.round(score * 100)}%`;
  };

  return (
    <div className="semantic-search" data-testid="semantic-search-panel">
      {/* Added: Search input form */}
      <form className="semantic-search__form" onSubmit={handleSearch}>
        <div className="semantic-search__input-wrapper">
          <Search size={16} />
          <input
            type="text"
            className="semantic-search__input"
            placeholder="Search by meaning (e.g., 'emails about project deadlines')..."
            value={queryInput}
            onChange={(e) => setQueryInput(e.target.value)}
            data-testid="semantic-search-input"
          />
        </div>
        <button
          type="submit"
          className="btn btn--primary semantic-search__btn"
          disabled={isSearching || queryInput.trim().length < 2}
          data-testid="semantic-search-btn"
        >
          {isSearching ? <Loader2 size={16} className="animate-spin" /> : <Search size={16} />}
          Search
        </button>
      </form>

      {/* Added: Search error display */}
      {searchError && (
        <div className="semantic-search__error" data-testid="semantic-search-error">
          {searchError}
        </div>
      )}

      {/* Added: Search results list */}
      {isSearching && (
        <div className="semantic-search__loading" data-testid="semantic-search-loading">
          <Loader2 size={20} className="animate-spin" />
          Searching by meaning...
        </div>
      )}

      {!isSearching && hasSearched && searchResults.length === 0 && !searchError && (
        <div className="semantic-search__empty" data-testid="semantic-search-empty">
          No similar emails found. Try a different query or index more emails.
        </div>
      )}

      {searchResults.length > 0 && (
        <div className="semantic-search__results" data-testid="semantic-search-results">
          <div className="semantic-search__results-header">
            {searchResults.length} result{searchResults.length !== 1 ? 's' : ''} found
          </div>
          {searchResults.map((result) => (
            <div
              key={`${result.folder}-${result.uid}`}
              className="semantic-search__result-row"
              onClick={() => handleResultClick(result)}
              role="button"
              tabIndex={0}
              onKeyDown={(e) => e.key === 'Enter' && handleResultClick(result)}
              data-testid="semantic-search-result-item"
            >
              <div className="semantic-search__result-subject">
                {result.subject || '(no subject)'}
              </div>
              <div className="semantic-search__result-meta">
                <span className="semantic-search__result-folder">{result.folder}</span>
                <span
                  className="semantic-search__result-score"
                  data-testid="semantic-search-score"
                >
                  {formatScore(result.similarity_score)} match
                </span>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Added: Index stats section */}
      <div className="semantic-search__stats">
        <button
          type="button"
          className="btn btn--secondary semantic-search__stats-btn"
          onClick={handleLoadStats}
          disabled={isLoadingStats}
          data-testid="semantic-search-stats-btn"
        >
          <Database size={14} />
          {isLoadingStats ? 'Loading...' : 'Index Stats'}
        </button>

        {indexStats && (
          <div className="semantic-search__stats-content" data-testid="semantic-search-stats">
            <div className="semantic-search__stats-total">
              Total indexed: <strong>{indexStats.total_indexed}</strong>
            </div>
            {indexStats.per_folder.length > 0 && (
              <div className="semantic-search__stats-folders">
                {indexStats.per_folder.map((folderStat) => (
                  <span key={folderStat.folder} className="semantic-search__stats-folder">
                    {folderStat.folder}: {folderStat.count}
                  </span>
                ))}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
