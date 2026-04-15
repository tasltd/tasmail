// Added: Unit tests for email models for TMAIL-143
import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/models/email.dart';

void main() {
  group('MobileMessageSummary', () {
    test('parses from JSON with all fields', () {
      final json = {
        'uid': 42,
        'folder': 'INBOX',
        'from': 'john@example.com',
        'subject': 'Test Email',
        'date': '2026-04-15T10:00:00Z',
        'is_read': true,
        'is_flagged': false,
        'has_attachment': true,
      };

      final msg = MobileMessageSummary.fromJson(json);
      expect(msg.uid, 42);
      expect(msg.folder, 'INBOX');
      expect(msg.from, 'john@example.com');
      expect(msg.subject, 'Test Email');
      expect(msg.isRead, true);
      expect(msg.isFlagged, false);
      expect(msg.hasAttachment, true);
    });

    test('handles missing optional fields', () {
      final json = {
        'uid': 1,
        'folder': 'INBOX',
      };

      final msg = MobileMessageSummary.fromJson(json);
      expect(msg.uid, 1);
      expect(msg.from, isNull);
      expect(msg.subject, isNull);
      expect(msg.date, isNull);
      expect(msg.isRead, false);
      expect(msg.isFlagged, false);
      expect(msg.hasAttachment, false);
    });
  });

  group('MobileMessageDetail', () {
    test('parses full message detail', () {
      final json = {
        'uid': 10,
        'folder': 'INBOX',
        'from': 'alice@example.com',
        'to': ['bob@example.com'],
        'cc': ['charlie@example.com'],
        'subject': 'Meeting Notes',
        'date': '2026-04-14T09:30:00Z',
        'body_html': '<p>Hello</p>',
        'body_text': 'Hello',
        'is_read': true,
        'is_flagged': true,
        'has_attachment': false,
        'attachments': [
          {
            'id': 'att-1',
            'filename': 'report.pdf',
            'content_type': 'application/pdf',
            'size_bytes': 1024,
          },
        ],
      };

      final msg = MobileMessageDetail.fromJson(json);
      expect(msg.uid, 10);
      expect(msg.from, 'alice@example.com');
      expect(msg.to, ['bob@example.com']);
      expect(msg.cc, ['charlie@example.com']);
      expect(msg.bodyHtml, '<p>Hello</p>');
      expect(msg.attachments.length, 1);
      expect(msg.attachments[0].filename, 'report.pdf');
      expect(msg.attachments[0].sizeBytes, 1024);
    });

    test('handles empty arrays and null body', () {
      final json = {
        'uid': 5,
        'folder': 'Sent',
      };

      final msg = MobileMessageDetail.fromJson(json);
      expect(msg.to, isEmpty);
      expect(msg.cc, isEmpty);
      expect(msg.bodyHtml, isNull);
      expect(msg.bodyText, isNull);
      expect(msg.attachments, isEmpty);
    });
  });

  group('MobileFolderSummary', () {
    test('parses folder summary', () {
      final json = {
        'name': 'INBOX',
        'unread_count': 5,
        'total_count': 100,
      };

      final folder = MobileFolderSummary.fromJson(json);
      expect(folder.name, 'INBOX');
      expect(folder.unreadCount, 5);
      expect(folder.totalCount, 100);
    });

    test('defaults counts to 0', () {
      final json = {'name': 'Drafts'};
      final folder = MobileFolderSummary.fromJson(json);
      expect(folder.unreadCount, 0);
      expect(folder.totalCount, 0);
    });
  });

  group('InboxResponse', () {
    test('parses inbox response with pagination', () {
      final json = {
        'messages': [
          {'uid': 1, 'folder': 'INBOX', 'subject': 'First'},
          {'uid': 2, 'folder': 'INBOX', 'subject': 'Second'},
        ],
        'total_count': 50,
        'page': 1,
        'per_page': 20,
      };

      final inbox = InboxResponse.fromJson(json);
      expect(inbox.messages.length, 2);
      expect(inbox.totalCount, 50);
      expect(inbox.page, 1);
      expect(inbox.perPage, 20);
      expect(inbox.messages[0].subject, 'First');
    });
  });

  group('AttachmentInfo', () {
    test('parses attachment info', () {
      final json = {
        'id': 'att-123',
        'filename': 'photo.jpg',
        'content_type': 'image/jpeg',
        'size_bytes': 2048576,
      };

      final att = AttachmentInfo.fromJson(json);
      expect(att.id, 'att-123');
      expect(att.filename, 'photo.jpg');
      expect(att.contentType, 'image/jpeg');
      expect(att.sizeBytes, 2048576);
    });

    test('defaults content type and size', () {
      final json = {
        'id': 'att-456',
        'filename': 'data.bin',
      };

      final att = AttachmentInfo.fromJson(json);
      expect(att.contentType, 'application/octet-stream');
      expect(att.sizeBytes, 0);
    });
  });
}
