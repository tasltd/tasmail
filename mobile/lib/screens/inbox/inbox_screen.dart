// Added: Inbox screen with message list for TMAIL-143
// PURPOSE: Shows email list with pull-to-refresh, infinite scroll, swipe actions
// EXTERNAL: Uses MailProvider for data, navigates to MessageScreen on tap

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../../models/email.dart';
import '../../providers/mail_provider.dart';
import '../../widgets/message_tile.dart';

class InboxScreen extends StatefulWidget {
  const InboxScreen({super.key});

  @override
  State<InboxScreen> createState() => _InboxScreenState();
}

class _InboxScreenState extends State<InboxScreen> {
  final ScrollController _scrollController = ScrollController();

  @override
  void initState() {
    super.initState();
    // Added: Load inbox on first build
    WidgetsBinding.instance.addPostFrameCallback((_) {
      final mail = context.read<MailProvider>();
      mail.loadInbox(refresh: true);
      mail.loadUnreadCount();
    });

    // Added: Infinite scroll listener
    _scrollController.addListener(_onScroll);
  }

  @override
  void dispose() {
    _scrollController.removeListener(_onScroll);
    _scrollController.dispose();
    super.dispose();
  }

  void _onScroll() {
    if (_scrollController.position.pixels >=
        _scrollController.position.maxScrollExtent - 200) {
      context.read<MailProvider>().loadMore();
    }
  }

  Future<void> _onRefresh() async {
    final mail = context.read<MailProvider>();
    await mail.loadInbox(refresh: true);
    await mail.loadUnreadCount();
  }

  void _onMessageTap(MobileMessageSummary message) {
    // Added: Mark as read and navigate to message view
    context.read<MailProvider>().markAsRead(message.folder, message.uid);
    Navigator.pushNamed(
      context,
      '/message',
      arguments: {'folder': message.folder, 'uid': message.uid},
    );
  }

  // Added: Swipe-to-delete handler
  Future<bool> _onDismissed(MobileMessageSummary message) async {
    final mail = context.read<MailProvider>();
    final deleted = await mail.deleteMessage(message.folder, message.uid);
    if (deleted && mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: const Text('Message deleted'),
          action: SnackBarAction(label: 'Undo', onPressed: () {
            // NOTE: Full undo would require server-side move back
            mail.loadInbox(refresh: true);
          }),
        ),
      );
    }
    return deleted;
  }

  // Added: Swipe-right-to-archive handler (TMAIL-54)
  // PURPOSE: Mirrors _onDismissed but moves the message to the Archive folder
  // instead of deleting. Returns true so Dismissible removes the tile only on
  // a successful server-side move.
  Future<bool> _onArchived(MobileMessageSummary message) async {
    final mail = context.read<MailProvider>();
    final archived = await mail.archiveMessage(message.folder, message.uid);
    if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(archived ? 'Message archived' : 'Archive failed'),
          action: archived
              ? SnackBarAction(
                  label: 'Undo',
                  onPressed: () {
                    // NOTE: Full undo would require server-side move back
                    mail.loadInbox(refresh: true);
                  },
                )
              : null,
        ),
      );
    }
    return archived;
  }

  @override
  Widget build(BuildContext context) {
    final mail = context.watch<MailProvider>();

    return Scaffold(
      appBar: AppBar(
        title: Text(mail.selectedFolder == 'INBOX'
            ? 'Inbox'
            : mail.selectedFolder),
      ),
      body: _buildBody(mail),
      floatingActionButton: FloatingActionButton(
        key: const Key('compose_fab'),
        onPressed: () => Navigator.pushNamed(context, '/compose'),
        child: const Icon(Icons.edit),
      ),
    );
  }

  Widget _buildBody(MailProvider mail) {
    if (mail.isLoadingInbox && mail.messages.isEmpty) {
      return const Center(child: CircularProgressIndicator());
    }

    if (mail.inboxError != null && mail.messages.isEmpty) {
      return Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            const Icon(Icons.error_outline, size: 48, color: Colors.grey),
            const SizedBox(height: 16),
            Text(mail.inboxError!),
            const SizedBox(height: 16),
            FilledButton(
              onPressed: () => mail.loadInbox(refresh: true),
              child: const Text('Retry'),
            ),
          ],
        ),
      );
    }

    if (mail.messages.isEmpty) {
      return Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(Icons.inbox_outlined, size: 64, color: Colors.grey.shade400),
            const SizedBox(height: 16),
            Text(
              'No messages',
              style: TextStyle(color: Colors.grey.shade600, fontSize: 16),
            ),
          ],
        ),
      );
    }

    return RefreshIndicator(
      onRefresh: _onRefresh,
      child: ListView.builder(
        controller: _scrollController,
        itemCount: mail.messages.length + (mail.hasMore ? 1 : 0),
        itemBuilder: (context, index) {
          if (index == mail.messages.length) {
            return const Padding(
              padding: EdgeInsets.all(16),
              child: Center(child: CircularProgressIndicator()),
            );
          }

          final message = mail.messages[index];
          return Dismissible(
            key: Key('${message.folder}-${message.uid}'),
            // Changed: bidirectional swipes for TMAIL-54
            //   endToStart  (swipe left)  -> delete  (red)
            //   startToEnd  (swipe right) -> archive (green)
            direction: DismissDirection.horizontal,
            background: Container(
              key: const Key('swipe_archive_bg'),
              color: Colors.green,
              alignment: Alignment.centerLeft,
              padding: const EdgeInsets.only(left: 16),
              child: const Icon(Icons.archive, color: Colors.white),
            ),
            secondaryBackground: Container(
              key: const Key('swipe_delete_bg'),
              color: Colors.red,
              alignment: Alignment.centerRight,
              padding: const EdgeInsets.only(right: 16),
              child: const Icon(Icons.delete, color: Colors.white),
            ),
            confirmDismiss: (direction) {
              if (direction == DismissDirection.startToEnd) {
                return _onArchived(message);
              }
              return _onDismissed(message);
            },
            child: MessageTile(
              message: message,
              onTap: () => _onMessageTap(message),
              onFlagToggle: () {
                mail.toggleFlag(message.folder, message.uid, !message.isFlagged);
              },
            ),
          );
        },
      ),
    );
  }
}
