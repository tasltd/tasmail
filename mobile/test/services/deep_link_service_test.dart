// Added: Unit tests for DeepLinkService.parseMailto for TMAIL-55
// PURPOSE: Validate every mailto: variant that browsers / contact apps /
//          calendar apps can hand us, since this is the only place we map
//          system intents into ComposePrefill.
import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/services/native/deep_link_service.dart';
import 'package:tasmail_mobile/services/native/share_intent_service.dart';

void main() {
  group('DeepLinkServiceImpl.parseMailto', () {
    test('returns null for non-mailto schemes', () {
      expect(
        DeepLinkServiceImpl.parseMailto(Uri.parse('https://example.com')),
        isNull,
      );
    });

    test('parses bare mailto:address', () {
      final p = DeepLinkServiceImpl.parseMailto(
              Uri.parse('mailto:alice@example.com'))
          as MailtoPrefill;

      expect(p.to, ['alice@example.com']);
      expect(p.cc, isEmpty);
      expect(p.bcc, isEmpty);
      expect(p.subject, isNull);
      expect(p.bodyText, isNull);
    });

    test('parses comma-separated recipients in path', () {
      final p = DeepLinkServiceImpl.parseMailto(
              Uri.parse('mailto:alice@example.com,bob@example.com'))
          as MailtoPrefill;

      expect(p.to, ['alice@example.com', 'bob@example.com']);
    });

    test('parses subject and body query params', () {
      final p = DeepLinkServiceImpl.parseMailto(
              Uri.parse('mailto:alice@example.com?subject=Hi&body=Hello'))
          as MailtoPrefill;

      expect(p.to, ['alice@example.com']);
      expect(p.subject, 'Hi');
      expect(p.bodyText, 'Hello');
    });

    test('parses cc and bcc when present in query', () {
      final p = DeepLinkServiceImpl.parseMailto(Uri.parse(
              'mailto:alice@example.com?cc=carol@example.com&bcc=dave@example.com'))
          as MailtoPrefill;

      expect(p.to, ['alice@example.com']);
      expect(p.cc, ['carol@example.com']);
      expect(p.bcc, ['dave@example.com']);
    });

    test('handles all-recipients-in-query form (empty path)', () {
      final p = DeepLinkServiceImpl.parseMailto(Uri.parse(
              'mailto:?to=alice@example.com&cc=carol@example.com&subject=X'))
          as MailtoPrefill;

      expect(p.to, ['alice@example.com']);
      expect(p.cc, ['carol@example.com']);
      expect(p.subject, 'X');
    });

    test('decodes URL-encoded subject and body', () {
      final p = DeepLinkServiceImpl.parseMailto(Uri.parse(
              'mailto:alice@example.com?subject=Hello%20world&body=Line%201%0ALine%202'))
          as MailtoPrefill;

      expect(p.subject, 'Hello world');
      expect(p.bodyText, 'Line 1\nLine 2');
    });

    test('isEmpty returns true for a truly empty prefill', () {
      final p = DeepLinkServiceImpl.parseMailto(Uri.parse('mailto:'))
          as MailtoPrefill;
      expect(p.isEmpty, isTrue);
    });

    test('isEmpty returns false when any recipient is present', () {
      final p = DeepLinkServiceImpl.parseMailto(Uri.parse('mailto:a@b.com'))
          as MailtoPrefill;
      expect(p.isEmpty, isFalse);
    });

    test('isEmpty returns false when only subject is present', () {
      final p = DeepLinkServiceImpl.parseMailto(
              Uri.parse('mailto:?subject=Hello'))
          as MailtoPrefill;
      expect(p.isEmpty, isFalse);
    });
  });

  group('ComposePrefill', () {
    test('isEmpty respects subject/body/attachments', () {
      expect(const ComposePrefill().isEmpty, isTrue);
      expect(const ComposePrefill(subject: 'x').isEmpty, isFalse);
      expect(const ComposePrefill(bodyText: 'x').isEmpty, isFalse);
    });
  });
}
