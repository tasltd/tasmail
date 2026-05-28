// Added: Widget tests for ComposeScreen native OS integrations (TMAIL-55).
// PURPOSE: Validate that the attach button, contact picker, and MailtoPrefill
//          handoff work without invoking real platform channels.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/models/attachment_draft.dart';
import 'package:tasmail_mobile/screens/compose/compose_screen.dart';
import 'package:tasmail_mobile/services/native/attachment_picker_service.dart';
import 'package:tasmail_mobile/services/native/contact_picker_service.dart';
import 'package:tasmail_mobile/services/native/deep_link_service.dart';

// NOTE: NoSplash.splashFactory avoids loading shaders/ink_sparkle.frag,
//       which fails to decode on Flutter 3.44+ (see existing compose_screen_test).
Widget _wrap({
  AttachmentPickerService? attachmentPicker,
  ContactPickerService? contactPicker,
  MailtoPrefill? prefill,
}) {
  return MaterialApp(
    theme: ThemeData(splashFactory: NoSplash.splashFactory),
    home: ComposeScreen(
      attachmentPicker: attachmentPicker,
      contactPicker: contactPicker,
      prefill: prefill,
    ),
  );
}

class _FakeAttachmentPicker implements AttachmentPickerService {
  final List<AttachmentDraft> galleryResult;
  final AttachmentDraft? cameraResult;
  final List<AttachmentDraft> filesResult;
  int cameraCalls = 0;
  int galleryCalls = 0;
  int fileCalls = 0;

  _FakeAttachmentPicker({
    this.galleryResult = const [],
    this.cameraResult,
    this.filesResult = const [],
  });

  @override
  Future<AttachmentDraft?> pickFromCamera() async {
    cameraCalls++;
    return cameraResult;
  }

  @override
  Future<List<AttachmentDraft>> pickFromGallery({bool allowMultiple = true}) async {
    galleryCalls++;
    return galleryResult;
  }

  @override
  Future<List<AttachmentDraft>> pickFromFiles({bool allowMultiple = true}) async {
    fileCalls++;
    return filesResult;
  }
}

class _FakeContactPicker implements ContactPickerService {
  final PickedContact? result;
  int calls = 0;

  _FakeContactPicker({this.result});

  @override
  Future<PickedContact?> pickContact() async {
    calls++;
    return result;
  }
}

void main() {
  group('ComposeScreen — attachment picker (TMAIL-55)', () {
    testWidgets('attach button is present in the app bar', (tester) async {
      await tester.pumpWidget(_wrap());
      expect(find.byKey(const Key('attach_button')), findsOneWidget);
    });

    testWidgets('tapping attach button shows the source bottom sheet',
        (tester) async {
      await tester.pumpWidget(_wrap(attachmentPicker: _FakeAttachmentPicker()));
      await tester.tap(find.byKey(const Key('attach_button')));
      await tester.pumpAndSettle();

      expect(find.byKey(const Key('attach_camera')), findsOneWidget);
      expect(find.byKey(const Key('attach_gallery')), findsOneWidget);
      expect(find.byKey(const Key('attach_file')), findsOneWidget);
    });

    testWidgets('picking from gallery appends chips for each file',
        (tester) async {
      final picker = _FakeAttachmentPicker(galleryResult: const [
        AttachmentDraft(
            fileName: 'a.jpg', filePath: '/tmp/a.jpg', sizeBytes: 1024),
        AttachmentDraft(
            fileName: 'b.jpg', filePath: '/tmp/b.jpg', sizeBytes: 2048),
      ]);
      await tester.pumpWidget(_wrap(attachmentPicker: picker));

      await tester.tap(find.byKey(const Key('attach_button')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('attach_gallery')));
      await tester.pumpAndSettle();

      expect(picker.galleryCalls, 1);
      expect(find.byKey(const Key('attachment_chip_0')), findsOneWidget);
      expect(find.byKey(const Key('attachment_chip_1')), findsOneWidget);
      // NOTE: chip label includes filename + human-readable size.
      expect(find.textContaining('a.jpg'), findsOneWidget);
      expect(find.textContaining('1.0 KB'), findsOneWidget);
    });

    testWidgets('picking from camera adds a single chip', (tester) async {
      final picker = _FakeAttachmentPicker(
        cameraResult: const AttachmentDraft(
            fileName: 'photo.jpg', filePath: '/tmp/photo.jpg', sizeBytes: 4096),
      );
      await tester.pumpWidget(_wrap(attachmentPicker: picker));

      await tester.tap(find.byKey(const Key('attach_button')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('attach_camera')));
      await tester.pumpAndSettle();

      expect(picker.cameraCalls, 1);
      expect(find.byKey(const Key('attachment_chip_0')), findsOneWidget);
      expect(find.textContaining('photo.jpg'), findsOneWidget);
    });

    testWidgets('picking from files adds chips and supports removal',
        (tester) async {
      final picker = _FakeAttachmentPicker(filesResult: const [
        AttachmentDraft(
            fileName: 'spec.pdf', filePath: '/tmp/spec.pdf', sizeBytes: 4096),
      ]);
      await tester.pumpWidget(_wrap(attachmentPicker: picker));

      await tester.tap(find.byKey(const Key('attach_button')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('attach_file')));
      await tester.pumpAndSettle();

      expect(picker.fileCalls, 1);
      expect(find.byKey(const Key('attachment_chip_0')), findsOneWidget);

      // Verify removal: tap the close icon on the chip.
      await tester.tap(find.byIcon(Icons.close));
      await tester.pumpAndSettle();
      expect(find.byKey(const Key('attachment_chip_0')), findsNothing);
    });

    testWidgets('cancelled picker does not add a chip', (tester) async {
      // All three pickers return nothing — user dismissed the OS UI.
      final picker = _FakeAttachmentPicker();
      await tester.pumpWidget(_wrap(attachmentPicker: picker));

      await tester.tap(find.byKey(const Key('attach_button')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(const Key('attach_gallery')));
      await tester.pumpAndSettle();

      expect(picker.galleryCalls, 1);
      expect(find.byKey(const Key('attachment_chip_0')), findsNothing);
    });
  });

  group('ComposeScreen — contact picker (TMAIL-55)', () {
    testWidgets('contact picker icon present on To field', (tester) async {
      await tester.pumpWidget(_wrap());
      expect(find.byKey(const Key('to_pick_contact')), findsOneWidget);
    });

    testWidgets('contact picker icon present on Cc once revealed',
        (tester) async {
      await tester.pumpWidget(_wrap());
      await tester.tap(find.text('Cc'));
      await tester.pump();
      expect(find.byKey(const Key('cc_pick_contact')), findsOneWidget);
    });

    testWidgets('picking a single-email contact fills To field', (tester) async {
      final picker = _FakeContactPicker(
        result: const PickedContact(
          displayName: 'Alice',
          emails: ['alice@example.com'],
        ),
      );
      await tester.pumpWidget(_wrap(contactPicker: picker));

      await tester.tap(find.byKey(const Key('to_pick_contact')));
      await tester.pumpAndSettle();

      expect(picker.calls, 1);
      final toField =
          tester.widget<TextField>(find.byKey(const Key('to_field')));
      expect(toField.controller?.text, 'alice@example.com');
    });

    testWidgets('cancelled contact picker leaves To field empty',
        (tester) async {
      final picker = _FakeContactPicker(result: null);
      await tester.pumpWidget(_wrap(contactPicker: picker));

      await tester.tap(find.byKey(const Key('to_pick_contact')));
      await tester.pumpAndSettle();

      final toField =
          tester.widget<TextField>(find.byKey(const Key('to_field')));
      expect(toField.controller?.text, '');
    });

    testWidgets('picking a contact appends to existing recipients',
        (tester) async {
      final picker = _FakeContactPicker(
        result: const PickedContact(
          displayName: 'Bob',
          emails: ['bob@example.com'],
        ),
      );
      await tester.pumpWidget(_wrap(contactPicker: picker));

      // Seed To with an existing recipient.
      await tester.enterText(
          find.byKey(const Key('to_field')), 'first@example.com');

      await tester.tap(find.byKey(const Key('to_pick_contact')));
      await tester.pumpAndSettle();

      final toField =
          tester.widget<TextField>(find.byKey(const Key('to_field')));
      expect(toField.controller?.text, 'first@example.com, bob@example.com');
    });
  });

  group('ComposeScreen — MailtoPrefill (TMAIL-55)', () {
    testWidgets('populates To/Cc/Subject/Body from MailtoPrefill',
        (tester) async {
      const prefill = MailtoPrefill(
        to: ['alice@example.com'],
        cc: ['carol@example.com'],
        subject: 'Hello',
        bodyText: 'Body text',
      );
      await tester.pumpWidget(_wrap(prefill: prefill));
      await tester.pump();

      final toField =
          tester.widget<TextField>(find.byKey(const Key('to_field')));
      expect(toField.controller?.text, 'alice@example.com');
      // Cc field gets auto-revealed because prefill has cc.
      final ccField =
          tester.widget<TextField>(find.byKey(const Key('cc_field')));
      expect(ccField.controller?.text, 'carol@example.com');
      final subjectField =
          tester.widget<TextField>(find.byKey(const Key('subject_field')));
      expect(subjectField.controller?.text, 'Hello');
      final bodyField =
          tester.widget<TextField>(find.byKey(const Key('body_field')));
      expect(bodyField.controller?.text, 'Body text');
    });

    testWidgets('folds Bcc into Cc when MailtoPrefill carries bcc',
        (tester) async {
      const prefill = MailtoPrefill(
        to: ['alice@example.com'],
        bcc: ['hidden@example.com'],
      );
      await tester.pumpWidget(_wrap(prefill: prefill));
      await tester.pump();

      final ccField =
          tester.widget<TextField>(find.byKey(const Key('cc_field')));
      expect(ccField.controller?.text, 'hidden@example.com');
    });
  });
}
