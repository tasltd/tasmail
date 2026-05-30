// Added (TMAIL-322): alt-UI search results page. Renders rows for messages
// returned by GET /api/search?q={query}&folder={folder?}. Reads the query
// from the hash URL search params so the Navbar can deep-link via
// `#/search?q=...` and a page reload preserves the query.
//
// Each row deep-links back to the EmailClient (root route) with
// ?folder=<folder>&uid=<uid> so EmailClient mounts pre-selecting the
// message in its reader pane. Folder/snippet/date are rendered alongside
// the subject so the user has enough context to click through.
//
// NOTE: keeps a single responsibility — presentation + data fetch only.
// The actual /api/search call lives in `api/messages.ts::searchMessages`
// (reused unchanged) so the wire shape stays a single source of truth.
import { useMemo } from 'react';
import { Link, useSearchParams } from 'react-router';
import { useQuery } from '@tanstack/react-query';
import { Search, AlertCircle } from 'lucide-react';
import { formatDistanceToNow } from 'date-fns';
import { searchMessages } from '@/api/messages';
import type { MessageEnvelope, SearchResponse } from '@/types/mail';

export function SearchResultsPage() {
  const [searchParams] = useSearchParams();
  const query = searchParams.get('q')?.trim() ?? '';
  const folder = searchParams.get('folder')?.trim() || undefined;

  // Disable the query when there's nothing to search for. Avoids firing a
  // backend request for an empty string (the backend would reject it via
  // validation::validate_search_query).
  const searchQuery = useQuery<SearchResponse>({
    queryKey: ['search', query, folder ?? null],
    queryFn: () => searchMessages(query, folder),
    enabled: query.length > 0,
  });

  const results: MessageEnvelope[] = useMemo(
    () => searchQuery.data?.messages ?? [],
    [searchQuery.data],
  );

  // Empty query state — invite the user to type a search term.
  if (query.length === 0) {
    return (
      <div
        data-testid="search-results-empty-query"
        className="flex flex-col items-center justify-center h-full text-center px-6 text-zinc-500"
      >
        <Search className="size-10 mb-3 opacity-50" />
        <p className="font-medium text-zinc-700 dark:text-zinc-300">
          Search your mail
        </p>
        <p className="text-sm mt-1">
          Type a query in the search bar above to find messages by subject,
          sender, or body content.
        </p>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col overflow-hidden">
      <div className="border-b border-zinc-200 dark:border-zinc-800 px-4 sm:px-6 py-3 shrink-0 bg-white dark:bg-zinc-950">
        <div className="flex items-baseline gap-2 flex-wrap">
          <h2 className="text-lg font-semibold">Search results</h2>
          <span
            className="text-sm text-zinc-500"
            data-testid="search-results-query"
          >
            for &ldquo;{query}&rdquo;
            {folder ? ` in ${folder}` : ''}
          </span>
          {searchQuery.isSuccess && (
            <span
              className="ml-auto text-sm text-zinc-500"
              data-testid="search-results-count"
            >
              {results.length} message{results.length === 1 ? '' : 's'}
            </span>
          )}
        </div>
      </div>

      <div className="flex-1 overflow-y-auto">
        {searchQuery.isLoading && (
          <div
            data-testid="search-results-loading"
            className="p-6 text-sm text-zinc-500"
          >
            Searching&hellip;
          </div>
        )}

        {searchQuery.isError && (
          <div
            data-testid="search-results-error"
            className="m-4 sm:m-6 p-4 rounded border border-red-200 dark:border-red-900 bg-red-50 dark:bg-red-950/40 text-sm text-red-700 dark:text-red-300 flex items-start gap-2"
          >
            <AlertCircle className="size-4 mt-0.5 shrink-0" />
            <div>
              Couldn&rsquo;t run the search. {String(searchQuery.error)}
            </div>
          </div>
        )}

        {searchQuery.isSuccess && results.length === 0 && (
          <div
            data-testid="search-results-empty"
            className="p-6 text-sm text-zinc-500"
          >
            No messages match &ldquo;{query}&rdquo;.
          </div>
        )}

        {searchQuery.isSuccess && results.length > 0 && (
          <ul data-testid="search-results-list" className="divide-y divide-zinc-200 dark:divide-zinc-800">
            {results.map((msg) => (
              <li key={`${searchQuery.data?.folder ?? folder ?? 'INBOX'}-${msg.uid}`}>
                <SearchResultRow
                  envelope={msg}
                  folder={searchQuery.data?.folder ?? folder ?? 'INBOX'}
                />
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

interface SearchResultRowProps {
  envelope: MessageEnvelope;
  folder: string;
}

function SearchResultRow({ envelope, folder }: SearchResultRowProps) {
  const ts = envelope.date ? new Date(envelope.date) : null;
  const tsLabel = ts && !isNaN(ts.getTime())
    ? formatDistanceToNow(ts, { addSuffix: true })
    : '';

  // Link back to EmailClient (root) with folder + uid so a click-through opens
  // the message in the reader pane. EmailClient reads those params on mount
  // (TMAIL-322 deep-link patch).
  const params = new URLSearchParams({ folder, uid: String(envelope.uid) });

  return (
    <Link
      to={`/?${params.toString()}`}
      data-testid="search-result-row"
      className="block px-4 sm:px-6 py-3 hover:bg-zinc-50 dark:hover:bg-zinc-900 transition-colors"
    >
      <div className="flex items-baseline justify-between gap-3">
        <span className="font-medium truncate">
          {envelope.from || '(unknown sender)'}
        </span>
        <span className="text-xs text-zinc-500 shrink-0">{tsLabel}</span>
      </div>
      <div className="text-sm text-zinc-700 dark:text-zinc-300 truncate mt-0.5">
        {envelope.subject || '(no subject)'}
      </div>
      <div className="text-xs text-zinc-500 mt-1">in {folder}</div>
    </Link>
  );
}
