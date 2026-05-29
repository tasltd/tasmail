// Added: NLP search history bottom sheet for TMAIL-147
// PURPOSE: Show prior natural language queries the user has run, with tap-to-
//          rerun and clear-all actions.
// EXTERNAL: Consumes NlpSearchHistoryEntry from models/nlp_search.dart and
//           SearchApi from api/search_api.dart.

import 'package:flutter/material.dart';
import '../api/search_api.dart';
import '../models/nlp_search.dart';

class SearchHistorySheet extends StatefulWidget {
  // PURPOSE: Receives the user's tap on a history row and pops the sheet.
  final void Function(NlpSearchHistoryEntry entry) onSelect;
  final SearchApi searchApi;

  const SearchHistorySheet({
    super.key,
    required this.searchApi,
    required this.onSelect,
  });

  @override
  State<SearchHistorySheet> createState() => _SearchHistorySheetState();
}

class _SearchHistorySheetState extends State<SearchHistorySheet> {
  // NOTE: Plain setState state rather than FutureBuilder — the clear-history
  //       flow needs to flip the view to "empty" in a single frame, which
  //       FutureBuilder's microtask-based rebuild doesn't guarantee under
  //       widget-test pumpAndSettle.
  List<NlpSearchHistoryEntry> _entries = const [];
  bool _loading = true;
  bool _failed = false;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final entries = await widget.searchApi.getNlpHistory();
      if (!mounted) return;
      setState(() {
        _entries = entries;
        _loading = false;
        _failed = false;
      });
    } catch (_) {
      if (!mounted) return;
      setState(() {
        _loading = false;
        _failed = true;
      });
    }
  }

  Future<void> _clearHistory() async {
    try {
      await widget.searchApi.clearNlpHistory();
      if (!mounted) return;
      setState(() => _entries = const []);
    } catch (_) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Failed to clear history')),
      );
    }
  }

  Widget _body(ThemeData theme) {
    if (_loading) {
      return const Padding(
        padding: EdgeInsets.all(24),
        child: Center(child: CircularProgressIndicator()),
      );
    }
    if (_failed) {
      return Padding(
        padding: const EdgeInsets.all(24),
        child: Text(
          'Failed to load history',
          style: TextStyle(color: theme.colorScheme.error),
        ),
      );
    }
    if (_entries.isEmpty) {
      return const Padding(
        key: Key('history_empty'),
        padding: EdgeInsets.all(24),
        child: Text('No prior searches yet'),
      );
    }
    return ListView.separated(
      shrinkWrap: true,
      itemCount: _entries.length,
      separatorBuilder: (_, _) => const Divider(height: 1),
      itemBuilder: (context, i) {
        final entry = _entries[i];
        return ListTile(
          key: Key('history_item_${entry.id}'),
          leading: const Icon(Icons.search),
          title: Text(entry.queryText),
          subtitle: Text(
            '${entry.resultCount} result${entry.resultCount == 1 ? '' : 's'}',
            style: theme.textTheme.bodySmall,
          ),
          onTap: () {
            Navigator.of(context).pop();
            widget.onSelect(entry);
          },
        );
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.fromLTRB(16, 12, 16, 16),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              children: [
                Icon(Icons.history, color: theme.colorScheme.primary),
                const SizedBox(width: 8),
                Text('Search history', style: theme.textTheme.titleMedium),
                const Spacer(),
                TextButton.icon(
                  key: const Key('clear_history_button'),
                  onPressed: _clearHistory,
                  icon: const Icon(Icons.delete_outline, size: 18),
                  label: const Text('Clear'),
                ),
              ],
            ),
            const Divider(),
            Flexible(child: _body(theme)),
          ],
        ),
      ),
    );
  }
}
