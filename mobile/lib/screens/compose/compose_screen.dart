// Added: Email compose screen for TMAIL-145
// PURPOSE: New email, reply, reply-all, forward with recipient fields and body editor
// EXTERNAL: Uses /api/messages/send endpoint via ApiClient

import 'package:flutter/material.dart';
import '../../api/api_client.dart';
import '../../models/email.dart';

class ComposeScreen extends StatefulWidget {
  final MobileMessageDetail? replyTo;
  final MobileMessageDetail? forward;

  const ComposeScreen({super.key, this.replyTo, this.forward});

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
  final ApiClient _api = ApiClient();

  @override
  void initState() {
    super.initState();
    _prefillForReplyOrForward();
  }

  // Added: Pre-fill fields for reply/forward
  void _prefillForReplyOrForward() {
    if (widget.replyTo != null) {
      final msg = widget.replyTo!;
      _toController.text = msg.from ?? '';
      _subjectController.text = msg.subject?.startsWith('Re:') == true
          ? msg.subject!
          : 'Re: ${msg.subject ?? ''}';
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
        'body_text': _bodyController.text,
        'body_html': '<p>${_bodyController.text.replaceAll('\n', '<br>')}</p>',
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

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text(widget.replyTo != null
            ? 'Reply'
            : widget.forward != null
                ? 'Forward'
                : 'Compose'),
        actions: [
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
            // Added: To field
            TextField(
              key: const Key('to_field'),
              controller: _toController,
              decoration: InputDecoration(
                labelText: 'To',
                border: const OutlineInputBorder(),
                suffixIcon: !_showCc
                    ? IconButton(
                        icon: const Text('Cc', style: TextStyle(fontSize: 12)),
                        onPressed: () => setState(() => _showCc = true),
                      )
                    : null,
              ),
              keyboardType: TextInputType.emailAddress,
            ),
            const SizedBox(height: 12),

            // Added: CC field (toggleable)
            if (_showCc) ...[
              TextField(
                key: const Key('cc_field'),
                controller: _ccController,
                decoration: const InputDecoration(
                  labelText: 'Cc',
                  border: OutlineInputBorder(),
                ),
                keyboardType: TextInputType.emailAddress,
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
