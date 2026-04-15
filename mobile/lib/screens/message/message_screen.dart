// Added: Message view screen for TMAIL-144
// PURPOSE: Full email view with HTML body rendering, headers, attachment list, reply/forward actions
// EXTERNAL: Uses MailProvider for data, flutter_widget_from_html for HTML rendering

import 'package:flutter/material.dart';
import 'package:flutter_widget_from_html/flutter_widget_from_html.dart';
import 'package:provider/provider.dart';
import '../../providers/mail_provider.dart';

class MessageScreen extends StatefulWidget {
  final String folder;
  final int uid;

  const MessageScreen({
    super.key,
    required this.folder,
    required this.uid,
  });

  @override
  State<MessageScreen> createState() => _MessageScreenState();
}

class _MessageScreenState extends State<MessageScreen> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      context.read<MailProvider>().loadMessage(widget.folder, widget.uid);
    });
  }

  @override
  Widget build(BuildContext context) {
    final mail = context.watch<MailProvider>();
    final message = mail.currentMessage;
    final theme = Theme.of(context);

    return Scaffold(
      appBar: AppBar(
        title: const Text('Message'),
        actions: [
          IconButton(
            icon: const Icon(Icons.reply),
            onPressed: message != null
                ? () => Navigator.pushNamed(context, '/compose', arguments: {
                      'replyTo': message,
                    })
                : null,
            tooltip: 'Reply',
          ),
          IconButton(
            icon: const Icon(Icons.forward),
            onPressed: message != null
                ? () => Navigator.pushNamed(context, '/compose', arguments: {
                      'forward': message,
                    })
                : null,
            tooltip: 'Forward',
          ),
          IconButton(
            icon: const Icon(Icons.delete_outline),
            onPressed: message != null
                ? () async {
                    final deleted = await mail.deleteMessage(
                      widget.folder,
                      widget.uid,
                    );
                    if (deleted && context.mounted) {
                      Navigator.pop(context);
                    }
                  }
                : null,
            tooltip: 'Delete',
          ),
        ],
      ),
      body: _buildBody(mail, message, theme),
    );
  }

  Widget _buildBody(MailProvider mail, dynamic message, ThemeData theme) {
    if (mail.isLoadingMessage) {
      return const Center(child: CircularProgressIndicator());
    }

    if (message == null) {
      return const Center(
        child: Text('Failed to load message'),
      );
    }

    return SingleChildScrollView(
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Added: Subject
          Text(
            message.subject ?? '(no subject)',
            style: theme.textTheme.titleLarge?.copyWith(
              fontWeight: FontWeight.bold,
            ),
          ),
          const SizedBox(height: 12),

          // Added: From/To/CC headers
          _buildHeader('From', message.from ?? 'Unknown', theme),
          if (message.to.isNotEmpty)
            _buildHeader('To', message.to.join(', '), theme),
          if (message.cc.isNotEmpty)
            _buildHeader('Cc', message.cc.join(', '), theme),
          if (message.date != null)
            _buildHeader('Date', _formatFullDate(message.date!), theme),
          const SizedBox(height: 8),

          // Added: Attachment chips
          if (message.attachments.isNotEmpty) ...[
            const Divider(),
            Wrap(
              spacing: 8,
              runSpacing: 4,
              children: message.attachments.map<Widget>((att) {
                return ActionChip(
                  avatar: const Icon(Icons.attach_file, size: 16),
                  label: Text(
                    att.filename,
                    style: const TextStyle(fontSize: 12),
                  ),
                  onPressed: () {
                    // NOTE: Attachment download handled in a future task
                    ScaffoldMessenger.of(context).showSnackBar(
                      SnackBar(content: Text('Downloading ${att.filename}...')),
                    );
                  },
                );
              }).toList(),
            ),
            const SizedBox(height: 8),
          ],

          const Divider(),
          const SizedBox(height: 8),

          // Added: HTML body rendering
          if (message.bodyHtml != null && message.bodyHtml!.isNotEmpty)
            HtmlWidget(
              message.bodyHtml!,
              textStyle: theme.textTheme.bodyMedium,
            )
          else if (message.bodyText != null)
            Text(message.bodyText!, style: theme.textTheme.bodyMedium)
          else
            Text(
              'No content',
              style: TextStyle(
                color: theme.colorScheme.onSurfaceVariant,
                fontStyle: FontStyle.italic,
              ),
            ),
        ],
      ),
    );
  }

  Widget _buildHeader(String label, String value, ThemeData theme) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 50,
            child: Text(
              label,
              style: TextStyle(
                color: theme.colorScheme.onSurfaceVariant,
                fontSize: 13,
                fontWeight: FontWeight.w500,
              ),
            ),
          ),
          Expanded(
            child: Text(
              value,
              style: const TextStyle(fontSize: 13),
            ),
          ),
        ],
      ),
    );
  }

  String _formatFullDate(String dateStr) {
    try {
      final date = DateTime.parse(dateStr);
      return '${date.day}/${date.month}/${date.year} ${date.hour.toString().padLeft(2, '0')}:${date.minute.toString().padLeft(2, '0')}';
    } catch (_) {
      return dateStr;
    }
  }
}
