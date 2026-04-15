// Added: Unit tests for sync models for TMAIL-151
import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/services/sync_service.dart';

void main() {
  group('SyncChange', () {
    test('parses new_message change', () {
      final json = {
        'change_type': 'new_message',
        'folder': 'INBOX',
        'uid': 42,
        'data': {'subject': 'New email', 'from': 'sender@example.com'},
      };

      final change = SyncChange.fromJson(json);
      expect(change.changeType, 'new_message');
      expect(change.folder, 'INBOX');
      expect(change.uid, 42);
      expect(change.data?['subject'], 'New email');
    });

    test('parses flag_change without data', () {
      final json = {
        'change_type': 'flag_change',
        'folder': 'INBOX',
        'uid': 10,
      };

      final change = SyncChange.fromJson(json);
      expect(change.changeType, 'flag_change');
      expect(change.data, isNull);
    });

    test('parses deletion change', () {
      final json = {
        'change_type': 'deletion',
        'folder': 'Trash',
        'uid': 5,
      };

      final change = SyncChange.fromJson(json);
      expect(change.changeType, 'deletion');
      expect(change.folder, 'Trash');
    });
  });

  group('SyncDelta', () {
    test('parses full delta response', () {
      final json = {
        'changes': [
          {'change_type': 'new_message', 'folder': 'INBOX', 'uid': 1},
          {'change_type': 'deletion', 'folder': 'INBOX', 'uid': 2},
        ],
        'checkpoint': '2026-04-15T10:00:00Z',
        'has_more': false,
      };

      final delta = SyncDelta.fromJson(json);
      expect(delta.changes.length, 2);
      expect(delta.checkpoint, '2026-04-15T10:00:00Z');
      expect(delta.hasMore, false);
    });

    test('defaults has_more to false', () {
      final json = {
        'changes': [],
        'checkpoint': '2026-04-15T10:00:00Z',
      };

      final delta = SyncDelta.fromJson(json);
      expect(delta.hasMore, false);
      expect(delta.changes, isEmpty);
    });
  });
}
