// Added: AttachmentDraft model for TMAIL-55
// PURPOSE: Represents a locally-picked file that is staged for upload as an
//          email attachment (from camera, gallery, file picker, or share intent).
// EXTERNAL: Used by AttachmentPickerService, ShareIntentService, ComposeScreen.

import 'dart:io';

class AttachmentDraft {
  final String fileName;
  final String filePath;
  final int sizeBytes;
  final String? mimeType;

  const AttachmentDraft({
    required this.fileName,
    required this.filePath,
    required this.sizeBytes,
    this.mimeType,
  });

  // Convenience factory: derive metadata from a File on disk.
  factory AttachmentDraft.fromFile(File file, {String? mimeType}) {
    final stat = file.statSync();
    return AttachmentDraft(
      fileName: file.uri.pathSegments.isNotEmpty
          ? file.uri.pathSegments.last
          : file.path,
      filePath: file.path,
      sizeBytes: stat.size,
      mimeType: mimeType,
    );
  }

  // NOTE: human-readable size used in attachment chips ("1.2 MB").
  String get displaySize {
    if (sizeBytes < 1024) return '$sizeBytes B';
    if (sizeBytes < 1024 * 1024) {
      return '${(sizeBytes / 1024).toStringAsFixed(1)} KB';
    }
    return '${(sizeBytes / (1024 * 1024)).toStringAsFixed(1)} MB';
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is AttachmentDraft &&
          fileName == other.fileName &&
          filePath == other.filePath &&
          sizeBytes == other.sizeBytes &&
          mimeType == other.mimeType;

  @override
  int get hashCode =>
      Object.hash(fileName, filePath, sizeBytes, mimeType);
}
