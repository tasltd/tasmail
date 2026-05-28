// Added: DeepLinkService for TMAIL-55
// PURPOSE: Resolve mailto: URIs (cold start + warm resume) into ComposePrefill
//          so the OS can hand off "send email to X" intents from browsers,
//          calendar apps, contact apps, etc. into TASMail.
// EXTERNAL: app_links plugin handles Android <intent-filter> + iOS Universal
//           Links / custom URL schemes; we just parse the resulting Uri.

import 'dart:async';

import 'package:app_links/app_links.dart';

import 'share_intent_service.dart';

abstract class DeepLinkService {
  Future<ComposePrefill?> initialLink();
  Stream<ComposePrefill> get incomingLinks;
}

class DeepLinkServiceImpl implements DeepLinkService {
  final AppLinks _appLinks;

  DeepLinkServiceImpl({AppLinks? appLinks}) : _appLinks = appLinks ?? AppLinks();

  @override
  Future<ComposePrefill?> initialLink() async {
    final uri = await _appLinks.getInitialLink();
    if (uri == null) return null;
    return parseMailto(uri);
  }

  @override
  Stream<ComposePrefill> get incomingLinks =>
      _appLinks.uriLinkStream.map(parseMailto).where((p) => p != null).cast();

  // Visible for testing — pure URI -> ComposePrefill conversion.
  //
  // Supports:
  //   mailto:alice@example.com
  //   mailto:alice@example.com,bob@example.com?subject=Hi&body=Hello
  //   mailto:?to=alice@example.com&cc=carol@example.com&subject=...&body=...
  //
  // Returns null for non-mailto schemes so callers can ignore them.
  static ComposePrefill? parseMailto(Uri uri) {
    if (uri.scheme.toLowerCase() != 'mailto') return null;

    final params = uri.queryParameters;
    // Path is everything between "mailto:" and "?"; can be empty if all
    // recipients live in ?to=.
    final pathRecipients = uri.path
        .split(',')
        .map((s) => s.trim())
        .where((s) => s.isNotEmpty)
        .toList();
    final queryTo = (params['to'] ?? '')
        .split(',')
        .map((s) => s.trim())
        .where((s) => s.isNotEmpty)
        .toList();

    final to = [...pathRecipients, ...queryTo];
    final cc = (params['cc'] ?? '')
        .split(',')
        .map((s) => s.trim())
        .where((s) => s.isNotEmpty)
        .toList();
    final bcc = (params['bcc'] ?? '')
        .split(',')
        .map((s) => s.trim())
        .where((s) => s.isNotEmpty)
        .toList();

    final subject = params['subject'];
    final body = params['body'];

    // Encode recipients into the body-text-free fields of ComposePrefill by
    // exposing extra fields via the MailtoPrefill subclass (so the compose
    // screen can pull To/Cc/Bcc out without us overloading ComposePrefill for
    // every consumer).
    return MailtoPrefill(
      to: to,
      cc: cc,
      bcc: bcc,
      subject: (subject == null || subject.isEmpty) ? null : subject,
      bodyText: (body == null || body.isEmpty) ? null : body,
    );
  }
}

// MailtoPrefill is a ComposePrefill that also carries To/Cc/Bcc lists, which
// only mailto: deep links populate (share-sheet payloads don't have these).
class MailtoPrefill extends ComposePrefill {
  final List<String> to;
  final List<String> cc;
  final List<String> bcc;

  const MailtoPrefill({
    this.to = const [],
    this.cc = const [],
    this.bcc = const [],
    super.subject,
    super.bodyText,
  }) : super(attachments: const []);

  @override
  bool get isEmpty =>
      to.isEmpty && cc.isEmpty && bcc.isEmpty && super.isEmpty;
}
