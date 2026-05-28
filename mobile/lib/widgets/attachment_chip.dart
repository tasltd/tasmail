// Added: AttachmentChip widget for TMAIL-55
// PURPOSE: Visual representation of a staged AttachmentDraft inside the compose
//          screen, with file name, human-readable size, and a delete handle.
// EXTERNAL: Used by ComposeScreen; no platform channels.

import 'package:flutter/material.dart';

import '../models/attachment_draft.dart';

class AttachmentChip extends StatelessWidget {
  final AttachmentDraft draft;
  final VoidCallback onRemove;

  const AttachmentChip({
    super.key,
    required this.draft,
    required this.onRemove,
  });

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(right: 8, bottom: 8),
      child: InputChip(
        avatar: const Icon(Icons.attach_file, size: 18),
        label: Text(
          '${draft.fileName} (${draft.displaySize})',
          overflow: TextOverflow.ellipsis,
        ),
        onDeleted: onRemove,
        deleteIcon: const Icon(Icons.close, size: 18),
      ),
    );
  }
}
