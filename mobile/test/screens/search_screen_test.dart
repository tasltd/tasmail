// Changed: Rewritten widget tests for SearchScreen for TMAIL-147.
// PURPOSE: Validate standard search, NLP search with parsed-params banner,
//          history sheet load + rerun + clear, and empty/error states using
//          an injected FakeSearchApi (no real Dio/network).
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/api/search_api.dart';
import 'package:tasmail_mobile/models/email.dart';
import 'package:tasmail_mobile/models/nlp_search.dart';
import 'package:tasmail_mobile/screens/search/search_screen.dart';

class _FakeSearchApi implements SearchApi {
  List<MobileMessageSummary> standardResults;
  NlpSearchResponse nlpResponse;
  List<NlpSearchHistoryEntry> history;
  Object? throwOnStandard;

  int standardCalls = 0;
  int nlpCalls = 0;
  int historyCalls = 0;
  int clearHistoryCalls = 0;
  String? lastStandardQuery;
  String? lastNlpQuery;

  _FakeSearchApi({
    this.standardResults = const [],
    NlpSearchResponse? nlpResponse,
    this.history = const [],
    this.throwOnStandard,
  }) : nlpResponse = nlpResponse ??
            const NlpSearchResponse(
              query: '',
              parsedParams: ParsedSearchParams(),
              resultCount: 0,
              results: [],
            );

  @override
  Future<List<MobileMessageSummary>> standardSearch({
    required String query,
    String folder = 'INBOX',
  }) async {
    standardCalls++;
    lastStandardQuery = query;
    if (throwOnStandard != null) throw throwOnStandard!;
    return standardResults;
  }

  @override
  Future<NlpSearchResponse> nlpSearch(String query) async {
    nlpCalls++;
    lastNlpQuery = query;
    return nlpResponse;
  }

  @override
  Future<List<NlpSearchHistoryEntry>> getNlpHistory() async {
    historyCalls++;
    return history;
  }

  @override
  Future<void> clearNlpHistory() async {
    clearHistoryCalls++;
    history = const [];
  }
}

Widget _wrap(SearchApi api) {
  // NOTE: NoSplash.splashFactory avoids loading shaders/ink_sparkle.frag,
  //       which fails to decode on Flutter 3.44+ (see
  //       docs/research/flutter-test-ink-sparkle-shader.md).
  return MaterialApp(
    theme: ThemeData(splashFactory: NoSplash.splashFactory),
    home: SearchScreen(searchApi: api),
  );
}

void main() {
  group('SearchScreen — basic layout', () {
    testWidgets('renders search input, AI toggle, and history button',
        (tester) async {
      await tester.pumpWidget(_wrap(_FakeSearchApi()));

      expect(find.byKey(const Key('search_input')), findsOneWidget);
      expect(find.text('AI Search'), findsOneWidget);
      expect(find.byKey(const Key('search_history_button')), findsOneWidget);
    });

    testWidgets('shows empty-state prompt before any search', (tester) async {
      await tester.pumpWidget(_wrap(_FakeSearchApi()));
      expect(find.text('Enter a search query'), findsOneWidget);
    });

    testWidgets('can type in the search field', (tester) async {
      await tester.pumpWidget(_wrap(_FakeSearchApi()));
      await tester.enterText(
        find.byKey(const Key('search_input')),
        'test query',
      );
      await tester.pump();
      expect(find.text('test query'), findsOneWidget);
    });
  });

  group('SearchScreen — standard search', () {
    testWidgets('submitting calls standardSearch and renders results',
        (tester) async {
      final api = _FakeSearchApi(standardResults: const [
        MobileMessageSummary(
          uid: 1,
          folder: 'INBOX',
          from: 'alice@example.com',
          subject: 'Welcome aboard',
          date: null,
          isRead: false,
          isFlagged: false,
          hasAttachment: false,
        ),
      ]);

      await tester.pumpWidget(_wrap(api));
      await tester.enterText(find.byKey(const Key('search_input')), 'welcome');
      await tester.tap(find.byKey(const Key('search_submit_button')));
      await tester.pumpAndSettle();

      expect(api.standardCalls, 1);
      expect(api.lastStandardQuery, 'welcome');
      expect(find.text('Welcome aboard'), findsOneWidget);
      // NOTE: AI banner must NOT show on standard search.
      expect(find.byKey(const Key('parsed_params_banner')), findsNothing);
    });

    testWidgets('shows error text when standardSearch throws', (tester) async {
      final api = _FakeSearchApi(throwOnStandard: Exception('boom'));

      await tester.pumpWidget(_wrap(api));
      await tester.enterText(find.byKey(const Key('search_input')), 'oops');
      await tester.tap(find.byKey(const Key('search_submit_button')));
      await tester.pumpAndSettle();

      expect(find.text('Search failed. Try again.'), findsOneWidget);
    });

    testWidgets('blank query does not call the API', (tester) async {
      final api = _FakeSearchApi();
      await tester.pumpWidget(_wrap(api));
      await tester.tap(find.byKey(const Key('search_submit_button')));
      await tester.pumpAndSettle();
      expect(api.standardCalls, 0);
      expect(api.nlpCalls, 0);
    });
  });

  group('SearchScreen — NLP search + parsed params', () {
    testWidgets('toggling AI Search and submitting calls nlpSearch',
        (tester) async {
      final api = _FakeSearchApi(
        nlpResponse: const NlpSearchResponse(
          query: 'emails from John about budget',
          parsedParams: ParsedSearchParams(
            from: 'John',
            subject: 'budget',
            keywords: ['quarterly'],
            dateFrom: '2026-01-01',
            hasAttachment: true,
          ),
          resultCount: 1,
          results: [
            NlpSearchResultItem(
              folder: 'INBOX',
              uid: 42,
              subject: 'Q1 budget review',
              from: 'john@example.com',
              date: null,
            ),
          ],
        ),
      );

      await tester.pumpWidget(_wrap(api));
      await tester.tap(find.text('AI Search'));
      await tester.pump();

      await tester.enterText(
        find.byKey(const Key('search_input')),
        'emails from John about budget',
      );
      await tester.tap(find.byKey(const Key('search_submit_button')));
      await tester.pumpAndSettle();

      expect(api.nlpCalls, 1);
      expect(api.standardCalls, 0);
      expect(api.lastNlpQuery, 'emails from John about budget');

      // Parsed-params banner is visible with the AI-extracted chips.
      expect(find.byKey(const Key('parsed_params_banner')), findsOneWidget);
      expect(find.text('from: John'), findsOneWidget);
      expect(find.text('subject: budget'), findsOneWidget);
      expect(find.text('quarterly'), findsOneWidget);
      expect(find.text('after 2026-01-01'), findsOneWidget);
      expect(find.text('has attachment'), findsOneWidget);

      // Result row renders via MessageTile.
      expect(find.text('Q1 budget review'), findsOneWidget);
    });

    testWidgets('empty parsed params hide the AI banner', (tester) async {
      final api = _FakeSearchApi(
        nlpResponse: const NlpSearchResponse(
          query: 'anything',
          parsedParams: ParsedSearchParams(),
          resultCount: 0,
          results: [],
        ),
      );

      await tester.pumpWidget(_wrap(api));
      await tester.tap(find.text('AI Search'));
      await tester.pump();
      await tester.enterText(find.byKey(const Key('search_input')), 'anything');
      await tester.tap(find.byKey(const Key('search_submit_button')));
      await tester.pumpAndSettle();

      expect(api.nlpCalls, 1);
      expect(find.byKey(const Key('parsed_params_banner')), findsNothing);
      expect(find.text('No results found'), findsOneWidget);
    });
  });

  group('SearchScreen — history sheet', () {
    testWidgets('opening history calls getNlpHistory and shows empty state',
        (tester) async {
      final api = _FakeSearchApi();
      await tester.pumpWidget(_wrap(api));

      await tester.tap(find.byKey(const Key('search_history_button')));
      await tester.pumpAndSettle();

      expect(api.historyCalls, 1);
      expect(find.byKey(const Key('history_empty')), findsOneWidget);
    });

    testWidgets('history entries render and clear button calls clearNlpHistory',
        (tester) async {
      final entry = NlpSearchHistoryEntry(
        id: 'h-1',
        queryText: 'invoices last month',
        parsedParams: const ParsedSearchParams(),
        resultCount: 3,
        createdAt: DateTime(2026, 5, 1),
      );
      final api = _FakeSearchApi(history: [entry]);

      await tester.pumpWidget(_wrap(api));
      await tester.tap(find.byKey(const Key('search_history_button')));
      await tester.pumpAndSettle();

      expect(find.byKey(const Key('history_item_h-1')), findsOneWidget);
      expect(find.text('invoices last month'), findsOneWidget);
      expect(find.text('3 results'), findsOneWidget);

      await tester.tap(find.byKey(const Key('clear_history_button')));
      await tester.pumpAndSettle();
      expect(api.clearHistoryCalls, 1);
      expect(find.byKey(const Key('history_empty')), findsOneWidget);
    });

    testWidgets('tapping a history row reruns the NLP search', (tester) async {
      final entry = NlpSearchHistoryEntry(
        id: 'h-2',
        queryText: 'meeting notes from last week',
        parsedParams: const ParsedSearchParams(subject: 'meeting'),
        resultCount: 0,
        createdAt: DateTime(2026, 5, 28),
      );
      final api = _FakeSearchApi(
        history: [entry],
        nlpResponse: const NlpSearchResponse(
          query: 'meeting notes from last week',
          parsedParams: ParsedSearchParams(subject: 'meeting'),
          resultCount: 0,
          results: [],
        ),
      );

      await tester.pumpWidget(_wrap(api));
      await tester.tap(find.byKey(const Key('search_history_button')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('history_item_h-2')));
      await tester.pumpAndSettle();

      expect(api.nlpCalls, 1);
      expect(api.lastNlpQuery, 'meeting notes from last week');
      // After rerun the search input should hold the rerun query.
      expect(find.text('meeting notes from last week'), findsWidgets);
      // Parsed banner from the rerun shows the AI's subject chip.
      expect(find.text('subject: meeting'), findsOneWidget);
    });
  });

  group('NlpSearchHistoryEntry.fromJson', () {
    test('parses a full row with nested parsed_params map', () {
      final entry = NlpSearchHistoryEntry.fromJson({
        'id': 'abc-1',
        'query_text': 'budget docs',
        'parsed_params': {
          'subject': 'budget',
          'keywords': ['Q1'],
          'has_attachment': true,
        },
        'result_count': 7,
        'created_at': '2026-05-29T10:00:00Z',
      });
      expect(entry.id, 'abc-1');
      expect(entry.queryText, 'budget docs');
      expect(entry.parsedParams.subject, 'budget');
      expect(entry.parsedParams.keywords, ['Q1']);
      expect(entry.parsedParams.hasAttachment, true);
      expect(entry.resultCount, 7);
      expect(entry.createdAt.year, 2026);
    });

    test('handles missing parsed_params gracefully', () {
      final entry = NlpSearchHistoryEntry.fromJson({
        'id': 'abc-2',
        'query_text': 'q',
        'result_count': 0,
        'created_at': '2026-05-29T10:00:00Z',
      });
      expect(entry.parsedParams.isEmpty, true);
    });
  });

  group('NlpSearchResponse.fromJson', () {
    test('parses backend shape with parsed_params + results', () {
      final resp = NlpSearchResponse.fromJson({
        'query': 'find emails',
        'parsed_params': {'from': 'alice'},
        'result_count': 1,
        'results': [
          {
            'folder': 'INBOX',
            'uid': 5,
            'subject': 'hi',
            'from': 'alice@example.com',
            'date': null,
          }
        ],
      });
      expect(resp.query, 'find emails');
      expect(resp.parsedParams.from, 'alice');
      expect(resp.resultCount, 1);
      expect(resp.results.single.uid, 5);
      expect(resp.results.single.from, 'alice@example.com');
    });
  });
}
