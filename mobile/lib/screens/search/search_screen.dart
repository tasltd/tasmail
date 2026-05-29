// Changed: Rewritten for TMAIL-147 to fix endpoint/parsing bugs and add
//          parsed-params display + search history.
// PURPOSE: Standard IMAP search via /api/search and AI natural-language search
//          via /api/search/nlp, with a banner showing what the AI parsed and a
//          history sheet of prior NLP queries.
// EXTERNAL: Delegates HTTP to SearchApi (search_api.dart), renders results via
//          MessageTile, and uses ParsedParamsBanner + SearchHistorySheet for the
//          two new pieces.

import 'package:flutter/material.dart';
import '../../api/search_api.dart';
import '../../models/email.dart';
import '../../models/nlp_search.dart';
import '../../widgets/message_tile.dart';
import '../../widgets/parsed_params_banner.dart';
import '../../widgets/search_history_sheet.dart';

class SearchScreen extends StatefulWidget {
  // PURPOSE: Tests inject a fake SearchApi here; production gets the default
  //          ApiClient-backed implementation.
  final SearchApi? searchApi;

  const SearchScreen({super.key, this.searchApi});

  @override
  State<SearchScreen> createState() => _SearchScreenState();
}

class _SearchScreenState extends State<SearchScreen> {
  final _searchController = TextEditingController();
  late final SearchApi _api;

  List<MobileMessageSummary> _results = const [];
  ParsedSearchParams? _lastParsedParams;
  bool _isSearching = false;
  bool _useNlp = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    _api = widget.searchApi ?? SearchApiClient();
  }

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }

  // PURPOSE: Convert NlpSearchResultItem → MobileMessageSummary so the same
  //          MessageTile can render both result kinds. NLP results lack
  //          read/flag/attachment info, so we default them.
  MobileMessageSummary _nlpItemToSummary(NlpSearchResultItem item) {
    return MobileMessageSummary(
      uid: item.uid,
      folder: item.folder,
      from: item.from,
      subject: item.subject,
      date: item.date,
      isRead: true,
      isFlagged: false,
      hasAttachment: false,
    );
  }

  Future<void> _performSearch() async {
    final query = _searchController.text.trim();
    if (query.isEmpty) return;

    setState(() {
      _isSearching = true;
      _error = null;
      _lastParsedParams = null;
    });

    try {
      if (_useNlp) {
        final response = await _api.nlpSearch(query);
        setState(() {
          _results = response.results.map(_nlpItemToSummary).toList();
          _lastParsedParams = response.parsedParams;
        });
      } else {
        final results = await _api.standardSearch(query: query);
        setState(() => _results = results);
      }
    } catch (_) {
      setState(() => _error = 'Search failed. Try again.');
    } finally {
      if (mounted) setState(() => _isSearching = false);
    }
  }

  void _openHistory() {
    showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      builder: (_) => SearchHistorySheet(
        searchApi: _api,
        onSelect: (entry) {
          _searchController.text = entry.queryText;
          setState(() => _useNlp = true);
          _performSearch();
        },
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final parsed = _lastParsedParams;

    return Scaffold(
      appBar: AppBar(
        title: const Text('Search'),
        actions: [
          IconButton(
            key: const Key('search_history_button'),
            icon: const Icon(Icons.history),
            tooltip: 'Search history',
            onPressed: _openHistory,
          ),
        ],
      ),
      body: Column(
        children: [
          Padding(
            padding: const EdgeInsets.all(16),
            child: Column(
              children: [
                TextField(
                  key: const Key('search_input'),
                  controller: _searchController,
                  decoration: InputDecoration(
                    hintText: _useNlp
                        ? 'e.g., "emails from John last week"'
                        : 'Search emails...',
                    prefixIcon: const Icon(Icons.search),
                    border: const OutlineInputBorder(),
                    suffixIcon: _searchController.text.isNotEmpty
                        ? IconButton(
                            icon: const Icon(Icons.clear),
                            onPressed: () {
                              _searchController.clear();
                              setState(() {
                                _results = const [];
                                _lastParsedParams = null;
                              });
                            },
                          )
                        : null,
                  ),
                  textInputAction: TextInputAction.search,
                  onSubmitted: (_) => _performSearch(),
                ),
                const SizedBox(height: 8),
                Row(
                  children: [
                    FilterChip(
                      label: const Text('AI Search'),
                      selected: _useNlp,
                      onSelected: (v) => setState(() => _useNlp = v),
                      avatar: Icon(
                        Icons.auto_awesome,
                        size: 16,
                        color: _useNlp
                            ? theme.colorScheme.onPrimaryContainer
                            : theme.colorScheme.onSurfaceVariant,
                      ),
                    ),
                    const Spacer(),
                    FilledButton.tonal(
                      key: const Key('search_submit_button'),
                      onPressed: _isSearching ? null : _performSearch,
                      child: _isSearching
                          ? const SizedBox(
                              width: 16,
                              height: 16,
                              child: CircularProgressIndicator(strokeWidth: 2),
                            )
                          : const Text('Search'),
                    ),
                  ],
                ),
              ],
            ),
          ),

          if (_error != null)
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16),
              child: Text(_error!, style: TextStyle(color: theme.colorScheme.error)),
            ),

          // Added: Show AI's parsed interpretation after an NLP search.
          if (parsed != null && !parsed.isEmpty)
            ParsedParamsBanner(params: parsed),

          Expanded(
            child: _results.isEmpty && !_isSearching
                ? Center(
                    child: Text(
                      _searchController.text.isEmpty
                          ? 'Enter a search query'
                          : 'No results found',
                      style: TextStyle(color: theme.colorScheme.onSurfaceVariant),
                    ),
                  )
                : ListView.builder(
                    itemCount: _results.length,
                    itemBuilder: (context, index) {
                      final msg = _results[index];
                      return MessageTile(
                        message: msg,
                        onTap: () {
                          Navigator.pushNamed(context, '/message', arguments: {
                            'folder': msg.folder,
                            'uid': msg.uid,
                          });
                        },
                      );
                    },
                  ),
          ),
        ],
      ),
    );
  }
}
