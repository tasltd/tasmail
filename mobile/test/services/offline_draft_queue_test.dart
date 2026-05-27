// Added: Tests for OfflineDraftQueue (TMAIL-51)
import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/services/offline_draft_queue.dart';

void main() {
  group('OfflineDraft', () {
    test('roundtrips through JSON', () {
      final draft = OfflineDraft(
        id: 'd1',
        to: 'a@example.com',
        subject: 'hi',
        body: 'hello there',
        attachmentPaths: ['/tmp/a.png'],
        createdAt: DateTime.utc(2026, 5, 1),
        attempts: 2,
        lastError: 'network',
      );
      final restored = OfflineDraft.fromJson(draft.toJson());
      expect(restored.id, draft.id);
      expect(restored.to, draft.to);
      expect(restored.subject, draft.subject);
      expect(restored.attachmentPaths, draft.attachmentPaths);
      expect(restored.attempts, 2);
      expect(restored.lastError, 'network');
    });
  });

  group('OfflineDraftQueue enqueue/remove', () {
    test('rejects duplicate ids and persists snapshot', () {
      late List<Map<String, dynamic>> snapshot;
      final queue = OfflineDraftQueue(onPersist: (s) => snapshot = s);

      queue.enqueue(OfflineDraft(id: 'd1', subject: 'one'));
      queue.enqueue(OfflineDraft(id: 'd1', subject: 'one-dup'));
      queue.enqueue(OfflineDraft(id: 'd2', subject: 'two'));

      expect(queue.length, 2);
      expect(snapshot.length, 2);
      expect(snapshot.first['id'], 'd1');
    });

    test('remove returns false when id is missing', () {
      final queue = OfflineDraftQueue();
      queue.enqueue(OfflineDraft(id: 'd1'));
      expect(queue.remove('missing'), isFalse);
      expect(queue.remove('d1'), isTrue);
      expect(queue.isEmpty, isTrue);
    });
  });

  group('OfflineDraftQueue.flush', () {
    test('removes successful sends and keeps failures up to the retry cap',
        () async {
      final queue = OfflineDraftQueue();
      queue.enqueue(OfflineDraft(id: 'd1'));
      queue.enqueue(OfflineDraft(id: 'd2'));
      queue.enqueue(OfflineDraft(id: 'd3'));

      final sent = await queue.flush((draft) async => draft.id != 'd2');

      expect(sent, 2);
      expect(queue.length, 1);
      expect(queue.pending.first.id, 'd2');
      expect(queue.pending.first.attempts, 1);
    });

    test('drops drafts that exceed kMaxOfflineDraftAttempts', () async {
      final queue = OfflineDraftQueue();
      queue.enqueue(OfflineDraft(id: 'd1'));

      for (int i = 0; i < kMaxOfflineDraftAttempts; i++) {
        await queue.flush((_) async => false);
      }

      expect(queue.isEmpty, isTrue);
    });

    test('captures thrown errors as lastError', () async {
      final queue = OfflineDraftQueue();
      queue.enqueue(OfflineDraft(id: 'd1'));

      await queue.flush((_) async => throw StateError('boom'));

      expect(queue.length, 1);
      expect(queue.pending.first.attempts, 1);
      expect(queue.pending.first.lastError, contains('boom'));
    });

    test('returns zero on empty queue without invoking sender', () async {
      final queue = OfflineDraftQueue();
      var called = false;
      final sent = await queue.flush((_) async {
        called = true;
        return true;
      });
      expect(sent, 0);
      expect(called, isFalse);
    });
  });

  group('OfflineDraftQueue.loadFromJson', () {
    test('rehydrates queue from snapshot', () {
      final queue = OfflineDraftQueue();
      queue.loadFromJson([
        {
          'id': 'd1',
          'subject': 'hi',
          'created_at': DateTime.utc(2026, 4, 1).toIso8601String(),
          'attempts': 0,
          'attachment_paths': <String>[],
        },
      ]);
      expect(queue.length, 1);
      expect(queue.pending.first.subject, 'hi');
    });
  });
}
