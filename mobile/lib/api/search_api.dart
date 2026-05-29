// Added: Typed search API client for TMAIL-147
// PURPOSE: Wraps the four backend search endpoints with strong types so the
//          search screen can stay UI-only and tests can swap in a fake.
// EXTERNAL: Calls /api/search, /api/search/nlp, /api/search/nlp/history.

import '../models/email.dart';
import '../models/nlp_search.dart';
import 'api_client.dart';

/// PURPOSE: Surface area the SearchScreen depends on. The screen never touches
///          ApiClient directly — that lets tests inject [FakeSearchApi].
abstract class SearchApi {
  // PURPOSE: Standard IMAP keyword search via /api/search.
  Future<List<MobileMessageSummary>> standardSearch({
    required String query,
    String folder = 'INBOX',
  });

  // PURPOSE: AI-parsed natural-language search via /api/search/nlp.
  Future<NlpSearchResponse> nlpSearch(String query);

  // PURPOSE: List the user's prior NLP queries via GET /api/search/nlp/history.
  Future<List<NlpSearchHistoryEntry>> getNlpHistory();

  // PURPOSE: Clear the user's NLP history via DELETE /api/search/nlp/history.
  Future<void> clearNlpHistory();
}

/// PURPOSE: Production implementation backed by the singleton ApiClient (Dio).
/// NOTE: Endpoint paths are deliberate — `/search/nlp` matches router.rs:656.
///       The previous mobile code called `/nlp-search`, which 404s.
class SearchApiClient implements SearchApi {
  final ApiClient _api;

  SearchApiClient({ApiClient? api}) : _api = api ?? ApiClient();

  @override
  Future<List<MobileMessageSummary>> standardSearch({
    required String query,
    String folder = 'INBOX',
  }) async {
    final response = await _api.get('/search', queryParams: {
      'q': query,
      'folder': folder,
    });
    // NOTE: /api/search returns {messages, total, query, folder}; the prior
    //       implementation tried to decode response.data as a List, which
    //       always failed.
    final body = response.data as Map<String, dynamic>;
    final raw = (body['messages'] as List<dynamic>?) ?? const [];
    return raw
        .map((m) => MobileMessageSummary.fromJson(m as Map<String, dynamic>))
        .toList();
  }

  @override
  Future<NlpSearchResponse> nlpSearch(String query) async {
    final response = await _api.post('/search/nlp', data: {'query': query});
    return NlpSearchResponse.fromJson(response.data as Map<String, dynamic>);
  }

  @override
  Future<List<NlpSearchHistoryEntry>> getNlpHistory() async {
    final response = await _api.get('/search/nlp/history');
    final raw = (response.data as List<dynamic>?) ?? const [];
    return raw
        .map((e) => NlpSearchHistoryEntry.fromJson(e as Map<String, dynamic>))
        .toList();
  }

  @override
  Future<void> clearNlpHistory() async {
    await _api.delete('/search/nlp/history');
  }
}
