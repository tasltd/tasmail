// Added: Unit tests for AttachmentDraft for TMAIL-55
import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/models/attachment_draft.dart';

void main() {
  group('AttachmentDraft', () {
    test('constructs with required fields', () {
      const draft = AttachmentDraft(
        fileName: 'report.pdf',
        filePath: '/tmp/report.pdf',
        sizeBytes: 2048,
        mimeType: 'application/pdf',
      );

      expect(draft.fileName, 'report.pdf');
      expect(draft.filePath, '/tmp/report.pdf');
      expect(draft.sizeBytes, 2048);
      expect(draft.mimeType, 'application/pdf');
    });

    test('displaySize formats bytes', () {
      expect(
        const AttachmentDraft(fileName: 'a', filePath: '/a', sizeBytes: 512)
            .displaySize,
        '512 B',
      );
    });

    test('displaySize formats kilobytes with one decimal', () {
      expect(
        const AttachmentDraft(fileName: 'a', filePath: '/a', sizeBytes: 2048)
            .displaySize,
        '2.0 KB',
      );
    });

    test('displaySize formats megabytes with one decimal', () {
      expect(
        const AttachmentDraft(
                fileName: 'a', filePath: '/a', sizeBytes: 5 * 1024 * 1024)
            .displaySize,
        '5.0 MB',
      );
    });

    test('equality compares by all fields', () {
      const a = AttachmentDraft(
          fileName: 'a.txt', filePath: '/a.txt', sizeBytes: 1, mimeType: 't');
      const b = AttachmentDraft(
          fileName: 'a.txt', filePath: '/a.txt', sizeBytes: 1, mimeType: 't');
      const c = AttachmentDraft(
          fileName: 'a.txt', filePath: '/a.txt', sizeBytes: 2, mimeType: 't');

      expect(a, b);
      expect(a.hashCode, b.hashCode);
      expect(a == c, isFalse);
    });
  });
}
