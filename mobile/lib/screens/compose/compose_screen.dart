// Added: Email compose screen for TMAIL-145
// Changed: TMAIL-55 — wired native OS integrations:
//   * AttachmentPickerService → camera / gallery / file-picker menu
//   * ContactPickerService    → system contact picker on To/Cc fields
//   * ComposePrefill arg      → ingest mailto: + share-sheet payloads
// Changed: TMAIL-145 — finished reply-all + contact autocomplete:
//   * ContactSuggestionService → typing-time overlay against /api/contacts
//   * replyAll arg            → prefill To=sender, Cc=original(to+cc)-self
//   * currentUserEmail arg    → used to exclude self from reply-all Cc
// PURPOSE: New email, reply, reply-all, forward with recipient fields, body
//          editor, and native attachments.
// EXTERNAL: Uses /api/messages/send endpoint via ApiClient.

import 'package:flutter/material.dart';
import '../../api/api_client.dart';
import '../../models/attachment_draft.dart';
import '../../models/email.dart';
import '../../services/contact_suggestion_service.dart';
import '../../services/native/attachment_picker_service.dart';
import '../../services/native/contact_picker_service.dart';
import '../../services/native/deep_link_service.dart';
import '../../services/native/share_intent_service.dart';
import '../../widgets/attachment_chip.dart';
import '../../widgets/email_recipient_field.dart';

class ComposeScreen extends StatefulWidget {
  final MobileMessageDetail? replyTo;
  // Added: TMAIL-145 — Reply-All flow: prefill To with the sender and Cc with
  //   the original recipients (minus the current user).
  final MobileMessageDetail? replyAll;
  final MobileMessageDetail? forward;
  // Added: TMAIL-55 — prefill from mailto: deep link or incoming share intent.
  final ComposePrefill? prefill;
  // Added: TMAIL-145 — current user's email, used to filter self out of the
  //   reply-all Cc list. Optional: when null, all original recipients are kept
  //   and the user can trim manually.
  final String? currentUserEmail;
  // Added: TMAIL-55 — DI seams for unit/widget tests.
  final AttachmentPickerService? attachmentPicker;
  final ContactPickerService? contactPicker;
  // Added: TMAIL-145 — DI seam for contact-autocomplete suggestions.
  final ContactSuggestionService? suggestionService;
  final ApiClient? api;

  const ComposeScreen({
    super.key,
    this.replyTo,
    this.replyAll,
    this.forward,
    this.prefill,
    this.currentUserEmail,
    this.attachmentPicker,
    this.contactPicker,
    this.suggestionService,
    this.api,
  });

  @override
  State<ComposeScreen> createState() => _ComposeScreenState();
}

class _ComposeScreenState extends State<ComposeScreen> {
  final _toController = TextEditingController();
  final _ccController = TextEditingController();
  final _subjectController = TextEditingController();
  final _bodyController = TextEditingController();
  bool _showCc = false;
  bool _isSending = false;
  // Added: TMAIL-55 — staged attachments awaiting send.
  final List<AttachmentDraft> _attachments = [];
  late final ApiClient _api;
  late final AttachmentPickerService _attachmentPicker;
  late final ContactPickerService _contactPicker;
  // Added: TMAIL-145 — autocomplete backend for To/Cc fields.
  late final ContactSuggestionService _suggestionService;

  @override
  void initState() {
    super.initState();
    _api = widget.api ?? ApiClient();
    _attachmentPicker = widget.attachmentPicker ?? AttachmentPickerServiceImpl();
    _contactPicker = widget.contactPicker ?? ContactPickerServiceImpl();
    _suggestionService =
        widget.suggestionService ?? ContactSuggestionServiceImpl(api: _api);
    _prefillForReplyOrForward();
    _applyPrefill(widget.prefill);
  }

  // Added: Pre-fill fields for reply/reply-all/forward.
  void _prefillForReplyOrForward() {
    if (widget.replyTo != null) {
      final msg = widget.replyTo!;
      _toController.text = msg.from ?? '';
      _subjectController.text = _withRePrefix(msg.subject);
      _bodyController.text =
          '\n\n--- Original Message ---\n${msg.bodyText ?? ''}';
    } else if (widget.replyAll != null) {
      // Added: TMAIL-145 — reply-all: To = original sender; Cc = original
      //   to + cc with the current user filtered out (so the user does not
      //   email themselves on reply).
      final msg = widget.replyAll!;
      _toController.text = msg.from ?? '';
      final ccCandidates = <String>[...msg.to, ...msg.cc];
      final cc = ccCandidates
          .map((e) => e.trim())
          .where((e) => e.isNotEmpty)
          .where((e) => !_isSelf(e, widget.currentUserEmail))
          .where((e) => !_isSelf(e, _extractEmail(msg.from)))
          .toList();
      if (cc.isNotEmpty) {
        _ccController.text = cc.join(', ');
        _showCc = true;
      }
      _subjectController.text = _withRePrefix(msg.subject);
      _bodyController.text =
          '\n\n--- Original Message ---\n${msg.bodyText ?? ''}';
    } else if (widget.forward != null) {
      final msg = widget.forward!;
      _subjectController.text = msg.subject?.startsWith('Fwd:') == true
          ? msg.subject!
          : 'Fwd: ${msg.subject ?? ''}';
      _bodyController.text =
          '\n\n--- Forwarded Message ---\nFrom: ${msg.from ?? ''}\nTo: ${msg.to.join(', ')}\nSubject: ${msg.subject ?? ''}\n\n${msg.bodyText ?? ''}';
    }
  }

  // PURPOSE: Idempotently prepend "Re: " — handles null and an existing
  //   prefix without doubling up.
  String _withRePrefix(String? subject) {
    final s = subject ?? '';
    return s.startsWith('Re:') ? s : 'Re: $s';
  }

  // PURPOSE: Pull the email out of "Display Name <user@host>" or accept a
  //   bare address. Case-insensitive comparison happens at the call site.
  String? _extractEmail(String? addr) {
    if (addr == null || addr.isEmpty) return null;
    final m = RegExp(r'<([^>]+)>').firstMatch(addr);
    return (m != null ? m.group(1)! : addr).trim();
  }

  // NOTE: Filters reply-all Cc against the current user AND the original
  //   sender (the sender already goes to the To: line, no need to Cc them).
  bool _isSelf(String addr, String? self) {
    if (self == null || self.isEmpty) return false;
    final lhs = _extractEmail(addr)?.toLowerCase();
    final rhs = _extractEmail(self)?.toLowerCase();
    return lhs != null && rhs != null && lhs == rhs;
  }

  // Added: TMAIL-55 — apply mailto: / share-sheet prefill on top of reply state.
  void _applyPrefill(ComposePrefill? prefill) {
    if (prefill == null || prefill.isEmpty) return;
    if (prefill is MailtoPrefill) {
      if (prefill.to.isNotEmpty) {
        _toController.text = prefill.to.join(', ');
      }
      if (prefill.cc.isNotEmpty) {
        _ccController.text = prefill.cc.join(', ');
        _showCc = true;
      }
      // NOTE: Bcc field isn't surfaced yet; fold it into Cc for now so it isn't
      //       silently dropped. UI for Bcc lives behind TMAIL-145 follow-up.
      if (prefill.bcc.isNotEmpty) {
        final existingCc =
            _ccController.text.isEmpty ? '' : '${_ccController.text}, ';
        _ccController.text = '$existingCc${prefill.bcc.join(', ')}';
        _showCc = true;
      }
    }
    if (prefill.subject != null && _subjectController.text.isEmpty) {
      _subjectController.text = prefill.subject!;
    }
    if (prefill.bodyText != null) {
      if (_bodyController.text.isEmpty) {
        _bodyController.text = prefill.bodyText!;
      } else {
        _bodyController.text = '${prefill.bodyText!}\n\n${_bodyController.text}';
      }
    }
    if (prefill.attachments.isNotEmpty) {
      _attachments.addAll(prefill.attachments);
    }
  }

  @override
  void dispose() {
    _toController.dispose();
    _ccController.dispose();
    _subjectController.dispose();
    _bodyController.dispose();
    super.dispose();
  }

  Future<void> _send() async {
    if (_toController.text.trim().isEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Please enter a recipient')),
      );
      return;
    }

    setState(() => _isSending = true);

    try {
      await _api.post('/messages/send', data: {
        'to': _toController.text
            .split(',')
            .map((e) => e.trim())
            .where((e) => e.isNotEmpty)
            .toList(),
        'cc': _showCc
            ? _ccController.text
                .split(',')
                .map((e) => e.trim())
                .where((e) => e.isNotEmpty)
                .toList()
            : [],
        'subject': _subjectController.text,
        // Fix: TMAIL-145 — backend SendRequest expects `text_body`/`html_body`
        //   (services/smtp_service.rs:12), not the old `body_text`/`body_html`.
        //   The wrong keys silently no-op'd so sent emails had empty bodies.
        'text_body': _bodyController.text,
        'html_body': '<p>${_bodyController.text.replaceAll('\n', '<br>')}</p>',
        // Added: TMAIL-55 — attachment metadata (file upload is handled by a
        //   second pass that POSTs each path to /api/attachments; for now we
        //   surface the count + names so the backend can warn about size).
        'attachments': _attachments
            .map((a) => {
                  'file_name': a.fileName,
                  'size_bytes': a.sizeBytes,
                  'local_path': a.filePath,
                })
            .toList(),
      });

      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Email sent')),
        );
        Navigator.pop(context);
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Failed to send email')),
        );
      }
    } finally {
      if (mounted) setState(() => _isSending = false);
    }
  }

  // Added: TMAIL-55 — attachment source bottom sheet (camera/gallery/files).
  Future<void> _showAttachmentSheet() async {
    final source = await showModalBottomSheet<_AttachmentSource>(
      context: context,
      builder: (context) => SafeArea(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            ListTile(
              key: const Key('attach_camera'),
              leading: const Icon(Icons.photo_camera),
              title: const Text('Take photo'),
              onTap: () => Navigator.pop(context, _AttachmentSource.camera),
            ),
            ListTile(
              key: const Key('attach_gallery'),
              leading: const Icon(Icons.photo_library),
              title: const Text('Choose from gallery'),
              onTap: () => Navigator.pop(context, _AttachmentSource.gallery),
            ),
            ListTile(
              key: const Key('attach_file'),
              leading: const Icon(Icons.folder_open),
              title: const Text('Pick a file'),
              onTap: () => Navigator.pop(context, _AttachmentSource.file),
            ),
          ],
        ),
      ),
    );
    if (source == null) return;
    await _pickAttachment(source);
  }

  Future<void> _pickAttachment(_AttachmentSource source) async {
    try {
      switch (source) {
        case _AttachmentSource.camera:
          final draft = await _attachmentPicker.pickFromCamera();
          if (draft != null && mounted) {
            setState(() => _attachments.add(draft));
          }
          break;
        case _AttachmentSource.gallery:
          final drafts = await _attachmentPicker.pickFromGallery();
          if (drafts.isNotEmpty && mounted) {
            setState(() => _attachments.addAll(drafts));
          }
          break;
        case _AttachmentSource.file:
          final drafts = await _attachmentPicker.pickFromFiles();
          if (drafts.isNotEmpty && mounted) {
            setState(() => _attachments.addAll(drafts));
          }
          break;
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Could not attach: $e')),
        );
      }
    }
  }

  // Added: TMAIL-55 — open system contact picker and append result to a field.
  Future<void> _pickContact(TextEditingController target) async {
    try {
      final picked = await _contactPicker.pickContact();
      if (picked == null || !picked.hasEmail) return;
      // If the contact has more than one email address, show a chooser; else
      // just take the first.
      String? email = picked.primaryEmail;
      if (picked.emails.length > 1 && mounted) {
        email = await showDialog<String>(
          context: context,
          builder: (ctx) => SimpleDialog(
            title: Text(picked.displayName ?? 'Choose email'),
            children: picked.emails
                .map((e) => SimpleDialogOption(
                      onPressed: () => Navigator.pop(ctx, e),
                      child: Text(e),
                    ))
                .toList(),
          ),
        );
      }
      if (email == null || !mounted) return;
      setState(() {
        final existing = target.text.trim();
        target.text = existing.isEmpty ? email! : '$existing, $email';
      });
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Could not pick contact: $e')),
        );
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text(widget.replyTo != null
            ? 'Reply'
            : widget.replyAll != null
                ? 'Reply All'
                : widget.forward != null
                    ? 'Forward'
                    : 'Compose'),
        actions: [
          // Added: TMAIL-55 — attach button in app bar.
          IconButton(
            key: const Key('attach_button'),
            icon: const Icon(Icons.attach_file),
            onPressed: _isSending ? null : _showAttachmentSheet,
            tooltip: 'Attach file',
          ),
          IconButton(
            key: const Key('send_button'),
            icon: _isSending
                ? const SizedBox(
                    width: 20,
                    height: 20,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.send),
            onPressed: _isSending ? null : _send,
            tooltip: 'Send',
          ),
        ],
      ),
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(16),
        child: Column(
          children: [
            // Added: TMAIL-145 — To field now wraps RawAutocomplete via
            //   EmailRecipientField. The `to_field` Key remains on the inner
            //   TextField, so existing widget tests keep working.
            EmailRecipientField(
              fieldKey: const Key('to_field'),
              controller: _toController,
              suggestionService: _suggestionService,
              label: 'To',
              prefixIcon: IconButton(
                key: const Key('to_pick_contact'),
                icon: const Icon(Icons.contacts),
                tooltip: 'Pick contact',
                onPressed: () => _pickContact(_toController),
              ),
              suffixIcon: !_showCc
                  ? IconButton(
                      icon: const Text('Cc', style: TextStyle(fontSize: 12)),
                      onPressed: () => setState(() => _showCc = true),
                    )
                  : null,
            ),
            const SizedBox(height: 12),

            // Added: CC field (toggleable) with contact picker + autocomplete.
            if (_showCc) ...[
              EmailRecipientField(
                fieldKey: const Key('cc_field'),
                controller: _ccController,
                suggestionService: _suggestionService,
                label: 'Cc',
                prefixIcon: IconButton(
                  key: const Key('cc_pick_contact'),
                  icon: const Icon(Icons.contacts),
                  tooltip: 'Pick contact',
                  onPressed: () => _pickContact(_ccController),
                ),
              ),
              const SizedBox(height: 12),
            ],

            // Added: Subject field
            TextField(
              key: const Key('subject_field'),
              controller: _subjectController,
              decoration: const InputDecoration(
                labelText: 'Subject',
                border: OutlineInputBorder(),
              ),
            ),
            const SizedBox(height: 12),

            // Added: TMAIL-55 — staged attachment chips.
            if (_attachments.isNotEmpty)
              Align(
                key: const Key('attachments_row'),
                alignment: Alignment.centerLeft,
                child: Wrap(
                  children: [
                    for (var i = 0; i < _attachments.length; i++)
                      AttachmentChip(
                        key: Key('attachment_chip_$i'),
                        draft: _attachments[i],
                        onRemove: () => setState(() => _attachments.removeAt(i)),
                      ),
                  ],
                ),
              ),
            if (_attachments.isNotEmpty) const SizedBox(height: 12),

            // Added: Body editor
            TextField(
              key: const Key('body_field'),
              controller: _bodyController,
              decoration: const InputDecoration(
                hintText: 'Write your message...',
                border: OutlineInputBorder(),
              ),
              maxLines: 15,
              minLines: 8,
              keyboardType: TextInputType.multiline,
            ),
          ],
        ),
      ),
    );
  }
}

enum _AttachmentSource { camera, gallery, file }
