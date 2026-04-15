// Added: Email search screen for TMAIL-147
// PURPOSE: Standard and NLP-powered email search with results display
// EXTERNAL: Uses /api/search and /api/nlp-search endpoints

import 'package:flutter/material.dart';
import '../../api/api_client.dart';
import '../../models/email.dart';
import '../../widgets/message_tile.dart';

class SearchScreen extends StatefulWidget {
  const SearchScreen({super.key});

  @override
  State<SearchScreen> createState() => _SearchScreenState();
}

class _SearchScreenState extends State<SearchScreen> {
  final _searchController = TextEditingController();
  final ApiClient _api = ApiClient();
  List<MobileMessageSummary> _results = [];
  bool _isSearching = false;
  bool _useNlp = false;
  String? _error;

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }

  Future<void> _performSearch() async {
    final query = _searchController.text.trim();
    if (query.isEmpty) return;

    setState(() {
      _isSearching = true;
      _error = null;
    });

    try {
      if (_useNlp) {
        // Added: NLP search via AI endpoint
        final response = await _api.post('/nlp-search', data: {
          'query': query,
        });
        final results = (response.data['results'] as List<dynamic>?)
                ?.map((r) => MobileMessageSummary.fromJson(r as Map<String, dynamic>))
                .toList() ??
            [];
        setState(() => _results = results);
      } else {
        // Added: Standard IMAP search
        final response = await _api.get('/search', queryParams: {
          'q': query,
          'folder': 'INBOX',
        });
        final results = (response.data as List<dynamic>?)
                ?.map((r) => MobileMessageSummary.fromJson(r as Map<String, dynamic>))
                .toList() ??
            [];
        setState(() => _results = results);
      }
    } catch (e) {
      setState(() => _error = 'Search failed. Try again.');
    } finally {
      setState(() => _isSearching = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return Scaffold(
      appBar: AppBar(
        title: const Text('Search'),
      ),
      body: Column(
        children: [
          // Added: Search input
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
                              setState(() => _results = []);
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
                    // Added: Toggle between standard and NLP search
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

          // Added: Error display
          if (_error != null)
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16),
              child: Text(_error!, style: TextStyle(color: theme.colorScheme.error)),
            ),

          // Added: Results list
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
