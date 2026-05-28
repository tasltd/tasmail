// Added: ShareIntentService for TMAIL-55
// PURPOSE: Listen for incoming Android/iOS share-sheet intents and translate
//          them into a ComposePrefill (subject/body + optional attachments).
//          Covers cold-start (app launched by a share) and warm-resume (app
//          already running, user shares again).
// EXTERNAL: share_handler plugin (uses ACTION_SEND on Android, NSItemProvider
//           on iOS) + path_provider for staging directories.

import 'dart:async';
import 'dart:io';

import 'package:share_handler/share_handler.dart';

import '../../models/attachment_draft.dart';

class ComposePrefill {
  final String? subject;
  final String? bodyText;
  final List<AttachmentDraft> attachments;

  const ComposePrefill({
    this.subject,
    this.bodyText,
    this.attachments = const [],
  });

  bool get isEmpty =>
      (subject == null || subject!.isEmpty) &&
      (bodyText == null || bodyText!.isEmpty) &&
      attachments.isEmpty;
}

abstract class ShareIntentService {
  // Initial share that launched the app (null if app was opened normally).
  Future<ComposePrefill?> initialShare();

  // Stream of warm-resume shares while the app is running.
  Stream<ComposePrefill> get incomingShares;
}

class ShareIntentServiceImpl implements ShareIntentService {
  // Injected so tests don't need the platform channel.
  final Future<SharedMedia?> Function() _getInitial;
  final Stream<SharedMedia> _stream;

  ShareIntentServiceImpl({
    Future<SharedMedia?> Function()? getInitial,
    Stream<SharedMedia>? stream,
  })  : _getInitial = getInitial ?? _defaultGetInitial,
        _stream = stream ?? _defaultStream();

  static Future<SharedMedia?> _defaultGetInitial() {
    return ShareHandler.instance.getInitialSharedMedia();
  }

  static Stream<SharedMedia> _defaultStream() {
    return ShareHandler.instance.sharedMediaStream;
  }

  @override
  Future<ComposePrefill?> initialShare() async {
    final media = await _getInitial();
    if (media == null) return null;
    return _toPrefill(media);
  }

  @override
  Stream<ComposePrefill> get incomingShares =>
      _stream.map(_toPrefill).where((p) => !p.isEmpty);

  // Visible for testing: pure conversion of a SharedMedia payload.
  static ComposePrefill _toPrefill(SharedMedia media) {
    final text = media.content?.trim();
    final attachments = (media.attachments ?? const <SharedAttachment?>[])
        .whereType<SharedAttachment>()
        .where((a) => a.path.isNotEmpty)
        .map((a) {
      final file = File(a.path);
      // NOTE: share_handler hands us paths in the app's cache dir already, so
      //       we don't need to copy. We just stat for size.
      final size = file.existsSync() ? file.statSync().size : 0;
      return AttachmentDraft(
        fileName: file.uri.pathSegments.isNotEmpty
            ? file.uri.pathSegments.last
            : a.path,
        filePath: a.path,
        sizeBytes: size,
        mimeType: null,
      );
    }).toList(growable: false);

    // If the share is a URL, plant it in the body. If it's a longer text
    // selection, also plant it in the body (subject stays blank — user fills).
    return ComposePrefill(
      subject: null,
      bodyText: (text == null || text.isEmpty) ? null : text,
      attachments: attachments,
    );
  }
}
