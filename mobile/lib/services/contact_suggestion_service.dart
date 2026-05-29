// Added: TMAIL-145 — recipient autocomplete backed by GET /api/contacts?q=<token>.
// PURPOSE: ComposeScreen's To/Cc fields call `suggest(token)` as the user types
//          and surfaces matches in an overlay. The backend already supports
//          substring search via `Contact::search` (email/display_name/company
//          ILIKE %q% LIMIT 50, see backend/src/models/contact.rs).
// EXTERNAL: ApiClient (dio) — Bearer token is injected by the existing
//           interceptor. Failures (network/401) are swallowed so autocomplete
//           never blocks the compose UI.

import '../api/api_client.dart';
import '../models/contact_suggestion.dart';

abstract class ContactSuggestionService {
  /// Returns up to N contact suggestions matching [query]. Should NEVER throw —
  /// autocomplete is a soft enhancement and must not break composing on a
  /// transient API error.
  Future<List<ContactSuggestion>> suggest(String query);
}

class ContactSuggestionServiceImpl implements ContactSuggestionService {
  final ApiClient _api;
  static const int _minQueryLength = 2;
  static const int _maxResults = 8;

  ContactSuggestionServiceImpl({ApiClient? api}) : _api = api ?? ApiClient();

  @override
  Future<List<ContactSuggestion>> suggest(String query) async {
    final trimmed = query.trim();
    if (trimmed.length < _minQueryLength) return const [];
    try {
      final resp = await _api.get('/contacts', queryParams: {'q': trimmed});
      final data = resp.data;
      if (data is! List) return const [];
      final results = data
          .whereType<Map<String, dynamic>>()
          .map(ContactSuggestion.fromJson)
          .where((s) => s.email.isNotEmpty)
          .take(_maxResults)
          .toList(growable: false);
      return results;
    } catch (_) {
      // NOTE: Fail-soft — overlay just shows nothing. Logged elsewhere by the
      //       Dio interceptor if it's a real outage.
      return const [];
    }
  }
}
