// Added: Unit tests for the TMAIL-145 ContactSuggestion model.

import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/models/contact_suggestion.dart';

void main() {
  group('ContactSuggestion.fromJson', () {
    test('parses email + display_name', () {
      final s = ContactSuggestion.fromJson({
        'email': 'alice@example.com',
        'display_name': 'Alice',
      });
      expect(s.email, 'alice@example.com');
      expect(s.displayName, 'Alice');
    });

    test('treats empty display_name as null', () {
      final s = ContactSuggestion.fromJson({
        'email': 'bob@example.com',
        'display_name': '',
      });
      expect(s.displayName, isNull);
    });

    test('handles missing display_name', () {
      final s = ContactSuggestion.fromJson({'email': 'carol@example.com'});
      expect(s.displayName, isNull);
      expect(s.email, 'carol@example.com');
    });

    test('trims whitespace on email', () {
      final s = ContactSuggestion.fromJson({'email': '  d@x.com  '});
      expect(s.email, 'd@x.com');
    });
  });

  group('ContactSuggestion.formatted', () {
    test('uses RFC 5322 form when display name is present', () {
      const s = ContactSuggestion(email: 'a@x.com', displayName: 'Alice');
      expect(s.formatted(), 'Alice <a@x.com>');
    });

    test('falls back to bare email when no display name', () {
      const s = ContactSuggestion(email: 'a@x.com');
      expect(s.formatted(), 'a@x.com');
    });

    test('falls back to bare email when display name is blank', () {
      const s = ContactSuggestion(email: 'a@x.com', displayName: '   ');
      expect(s.formatted(), 'a@x.com');
    });
  });

  group('ContactSuggestion equality', () {
    test('values are equal when email + name match', () {
      const a = ContactSuggestion(email: 'a@x.com', displayName: 'A');
      const b = ContactSuggestion(email: 'a@x.com', displayName: 'A');
      expect(a, equals(b));
      expect(a.hashCode, equals(b.hashCode));
    });

    test('differs when email differs', () {
      const a = ContactSuggestion(email: 'a@x.com');
      const b = ContactSuggestion(email: 'b@x.com');
      expect(a, isNot(equals(b)));
    });
  });
}
