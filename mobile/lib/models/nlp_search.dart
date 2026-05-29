// Added: NLP search data models for TMAIL-147
// PURPOSE: Dart mirrors of backend ParsedSearchParams / NlpSearchResponse /
//          NlpSearchResultItem / NlpSearchHistory so the search screen can
//          surface what the AI parsed and the user's prior queries.
// EXTERNAL: Maps to backend/src/models/nlp_search.rs

/// PURPOSE: AI-parsed structured parameters extracted from a free-text query.
/// NOTE: Every field is optional — the AI may extract a subset.
class ParsedSearchParams {
  final String? from;
  final String? to;
  final String? subject;
  final List<String> keywords;
  final String? dateFrom;
  final String? dateTo;
  final String? folder;
  final bool? hasAttachment;

  const ParsedSearchParams({
    this.from,
    this.to,
    this.subject,
    this.keywords = const [],
    this.dateFrom,
    this.dateTo,
    this.folder,
    this.hasAttachment,
  });

  factory ParsedSearchParams.fromJson(Map<String, dynamic> json) {
    return ParsedSearchParams(
      from: json['from'] as String?,
      to: json['to'] as String?,
      subject: json['subject'] as String?,
      keywords: (json['keywords'] as List<dynamic>?)
              ?.map((k) => k.toString())
              .toList() ??
          const [],
      dateFrom: json['date_from'] as String?,
      dateTo: json['date_to'] as String?,
      folder: json['folder'] as String?,
      hasAttachment: json['has_attachment'] as bool?,
    );
  }

  // PURPOSE: Used by the parsed-params banner to decide whether to render at all.
  bool get isEmpty =>
      from == null &&
      to == null &&
      subject == null &&
      keywords.isEmpty &&
      dateFrom == null &&
      dateTo == null &&
      folder == null &&
      hasAttachment == null;
}

/// PURPOSE: A single NLP search result row. Distinct from MobileMessageSummary —
///          backend's NlpSearchResultItem does NOT carry is_read/is_flagged/
///          has_attachment, so we keep a minimal struct that only models what
///          the endpoint actually returns.
class NlpSearchResultItem {
  final String folder;
  final int uid;
  final String? subject;
  final String? from;
  final String? date;

  const NlpSearchResultItem({
    required this.folder,
    required this.uid,
    this.subject,
    this.from,
    this.date,
  });

  factory NlpSearchResultItem.fromJson(Map<String, dynamic> json) {
    return NlpSearchResultItem(
      folder: json['folder'] as String,
      uid: json['uid'] as int,
      subject: json['subject'] as String?,
      from: json['from'] as String?,
      date: json['date'] as String?,
    );
  }
}

/// PURPOSE: Wraps the full /api/search/nlp response so callers see both the
///          parsed params (for the banner) and the result rows.
class NlpSearchResponse {
  final String query;
  final ParsedSearchParams parsedParams;
  final int resultCount;
  final List<NlpSearchResultItem> results;

  const NlpSearchResponse({
    required this.query,
    required this.parsedParams,
    required this.resultCount,
    required this.results,
  });

  factory NlpSearchResponse.fromJson(Map<String, dynamic> json) {
    return NlpSearchResponse(
      query: json['query'] as String? ?? '',
      parsedParams: ParsedSearchParams.fromJson(
        (json['parsed_params'] as Map<String, dynamic>?) ?? const {},
      ),
      resultCount: json['result_count'] as int? ?? 0,
      results: (json['results'] as List<dynamic>?)
              ?.map((r) => NlpSearchResultItem.fromJson(r as Map<String, dynamic>))
              .toList() ??
          const [],
    );
  }
}

/// PURPOSE: One entry from GET /api/search/nlp/history. The backend stores
///          parsed_params as JSONB; we round-trip it through ParsedSearchParams
///          so the history sheet can show the same banner if needed.
class NlpSearchHistoryEntry {
  final String id;
  final String queryText;
  final ParsedSearchParams parsedParams;
  final int resultCount;
  final DateTime createdAt;

  const NlpSearchHistoryEntry({
    required this.id,
    required this.queryText,
    required this.parsedParams,
    required this.resultCount,
    required this.createdAt,
  });

  factory NlpSearchHistoryEntry.fromJson(Map<String, dynamic> json) {
    final parsedRaw = json['parsed_params'];
    final parsedMap = parsedRaw is Map<String, dynamic>
        ? parsedRaw
        : const <String, dynamic>{};
    return NlpSearchHistoryEntry(
      id: json['id'].toString(),
      queryText: json['query_text'] as String? ?? '',
      parsedParams: ParsedSearchParams.fromJson(parsedMap),
      resultCount: (json['result_count'] as num?)?.toInt() ?? 0,
      createdAt: DateTime.tryParse(json['created_at']?.toString() ?? '') ??
          DateTime.fromMillisecondsSinceEpoch(0),
    );
  }
}
