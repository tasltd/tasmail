// Added: AttachmentPickerService for TMAIL-55
// PURPOSE: Single entry point that wraps camera, photo gallery, and document
//          (SAF / Files app) pickers. Returns AttachmentDraft list so the caller
//          (ComposeScreen) doesn't care which native source produced the file.
// EXTERNAL: image_picker (camera + gallery), file_picker (documents).

import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:image_picker/image_picker.dart';

import '../../models/attachment_draft.dart';

abstract class AttachmentPickerService {
  // Capture a single photo via the device camera.
  Future<AttachmentDraft?> pickFromCamera();

  // Pick one or more images from the gallery.
  Future<List<AttachmentDraft>> pickFromGallery({bool allowMultiple = true});

  // Pick arbitrary documents via the system file picker (SAF / Files app).
  Future<List<AttachmentDraft>> pickFromFiles({bool allowMultiple = true});
}

class AttachmentPickerServiceImpl implements AttachmentPickerService {
  // Allow tests to inject doubles instead of hitting native platform channels.
  final ImagePicker _imagePicker;
  final Future<FilePickerResult?> Function({
    required FileType type,
    required bool allowMultiple,
  }) _filePicker;

  AttachmentPickerServiceImpl({
    ImagePicker? imagePicker,
    Future<FilePickerResult?> Function({
      required FileType type,
      required bool allowMultiple,
    })? filePicker,
  })  : _imagePicker = imagePicker ?? ImagePicker(),
        _filePicker = filePicker ?? _defaultFilePicker;

  static Future<FilePickerResult?> _defaultFilePicker({
    required FileType type,
    required bool allowMultiple,
  }) {
    return FilePicker.platform.pickFiles(
      type: type,
      allowMultiple: allowMultiple,
      withData: false,
    );
  }

  @override
  Future<AttachmentDraft?> pickFromCamera() async {
    final XFile? photo = await _imagePicker.pickImage(
      source: ImageSource.camera,
      // NOTE: cap at 2048px and 85% quality so we don't post 12 MB photos
      //       straight out of a flagship camera over a 4G connection.
      maxWidth: 2048,
      maxHeight: 2048,
      imageQuality: 85,
    );
    if (photo == null) return null;
    return AttachmentDraft.fromFile(File(photo.path), mimeType: photo.mimeType);
  }

  @override
  Future<List<AttachmentDraft>> pickFromGallery({
    bool allowMultiple = true,
  }) async {
    if (!allowMultiple) {
      final XFile? single = await _imagePicker.pickImage(
        source: ImageSource.gallery,
        maxWidth: 2048,
        maxHeight: 2048,
        imageQuality: 85,
      );
      if (single == null) return const [];
      return [AttachmentDraft.fromFile(File(single.path), mimeType: single.mimeType)];
    }
    final List<XFile> images = await _imagePicker.pickMultiImage(
      maxWidth: 2048,
      maxHeight: 2048,
      imageQuality: 85,
    );
    return images
        .map((x) => AttachmentDraft.fromFile(File(x.path), mimeType: x.mimeType))
        .toList(growable: false);
  }

  @override
  Future<List<AttachmentDraft>> pickFromFiles({
    bool allowMultiple = true,
  }) async {
    final FilePickerResult? result = await _filePicker(
      type: FileType.any,
      allowMultiple: allowMultiple,
    );
    if (result == null) return const [];
    return result.files
        .where((f) => f.path != null)
        .map((f) => AttachmentDraft(
              fileName: f.name,
              filePath: f.path!,
              sizeBytes: f.size,
              // NOTE: file_picker exposes extension, not mime; let the backend
              //       sniff Content-Type via the filename on upload.
              mimeType: null,
            ))
        .toList(growable: false);
  }
}
