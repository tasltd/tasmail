// Added: Inbox screen with message list for TMAIL-143
// Changed: TMAIL-148 — swipe gestures are now driven by SwipeActionsService so
//          the user can rebind left/right swipe to archive, delete, mark unread,
//          star-toggle, or none from the settings screen.
// PURPOSE: Shows email list with pull-to-refresh, infinite scroll, swipe actions
// EXTERNAL: Uses MailProvider for data, navigates to MessageScreen on tap

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../../models/email.dart';
import '../../providers/mail_provider.dart';
import '../../services/swipe_actions_service.dart';
import '../../services/swipe_preferences.dart';
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
      if (!mounted) return;
      final mail = context.read<MailProvider>();
      mail.loadInbox(refresh: true);
      mail.loadUnreadCount();
      // Added: Hydrate swipe-action prefs from secure storage if a service is
      // provided in the widget tree. The provider is optional so existing
      // tests that don't wire it keep working with the defaults.
      final swipe = _readSwipeService();
      if (swipe != null && !swipe.isLoaded) {
        swipe.load();
      }
    });

    // Added: Infinite scroll listener
    _scrollController.addListener(_onScroll);
  }

  // Added: TMAIL-148 — optional Provider lookup that never throws when the
  // SwipeActionsService isn't wired into the widget tree (legacy tests, hot-
  // reload before the provider mounts).
  SwipeActionsService? _readSwipeService() {
    try {
      return Provider.of<SwipeActionsService>(context, listen: false);
    } catch (_) {
      return null;
    }
  }

  SwipeActionsService? _watchSwipeService() {
    try {
      return Provider.of<SwipeActionsService>(context, listen: true);
    } catch (_) {
      return null;
    }
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

  // Added: TMAIL-148 — central dispatcher for a configured swipe action.
  // PURPOSE: Translates a SwipeAction enum into the matching MailProvider call,
  //          shows the right SnackBar, and returns `true` only when Dismissible
  //          should actually remove the tile. Non-destructive actions
  //          (markUnread, toggleFlag) always return `false` so the tile stays
  //          in place after the swipe.
  Future<bool> _performAction(
    SwipeAction action,
    MobileMessageSummary message,
  ) async {
    final mail = context.read<MailProvider>();
    switch (action) {
      case SwipeAction.none:
        return false;
      case SwipeAction.archive:
        final ok = await mail.archiveMessage(message.folder, message.uid);
        if (mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(
              content: Text(ok ? 'Message archived' : 'Archive failed'),
              action: ok
                  ? SnackBarAction(
                      label: 'Undo',
                      onPressed: () => mail.loadInbox(refresh: true),
                    )
                  : null,
            ),
          );
        }
        return ok;
      case SwipeAction.delete:
        final ok = await mail.deleteMessage(message.folder, message.uid);
        if (ok && mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(
              content: const Text('Message deleted'),
              action: SnackBarAction(
                label: 'Undo',
                onPressed: () => mail.loadInbox(refresh: true),
              ),
            ),
          );
        }
        return ok;
      case SwipeAction.markUnread:
        final ok = await mail.markAsUnread(message.folder, message.uid);
        if (mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(
              content: Text(ok ? 'Marked as unread' : 'Mark unread failed'),
            ),
          );
        }
        // NOTE: tile stays — message still lives in the same folder.
        return false;
      case SwipeAction.toggleFlag:
        final next = !message.isFlagged;
        final ok = await mail.setFlagged(message.folder, message.uid, next);
        if (mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(
              content: Text(
                ok
                    ? (next ? 'Message starred' : 'Star removed')
                    : 'Update failed',
              ),
            ),
          );
        }
        return false;
    }
  }

  // Added: TMAIL-148 — picks the visual background for each direction based on
  // the configured action so the colour cue matches the actual outcome.
  Color _backgroundColor(SwipeAction action) => switch (action) {
        SwipeAction.archive => Colors.green,
        SwipeAction.delete => Colors.red,
        SwipeAction.markUnread => Colors.blueGrey,
        SwipeAction.toggleFlag => Colors.amber,
        SwipeAction.none => Colors.transparent,
      };

  IconData _backgroundIcon(SwipeAction action) => switch (action) {
        SwipeAction.archive => Icons.archive,
        SwipeAction.delete => Icons.delete,
        SwipeAction.markUnread => Icons.mark_email_unread,
        SwipeAction.toggleFlag => Icons.star,
        SwipeAction.none => Icons.block,
      };

  @override
  Widget build(BuildContext context) {
    final mail = context.watch<MailProvider>();
    // Added: Watch the swipe service (if present) so the InboxScreen rebuilds
    //        when the user picks a new action in settings.
    final swipe = _watchSwipeService();
    final prefs = swipe?.preferences ?? const SwipePreferences();

    // Changed: TMAIL-149 — the Compose FAB now lives on the HomeScreen
    //          Scaffold (so it's visible across all bottom-nav tabs, not just
    //          the Inbox). Keeping a second FAB here caused two widgets to
    //          share the `compose_fab` key when InboxScreen rendered inside
    //          HomeScreen's IndexedStack.
    return Scaffold(
      appBar: AppBar(
        title: Text(mail.selectedFolder == 'INBOX'
            ? 'Inbox'
            : mail.selectedFolder),
      ),
      body: _buildBody(mail, prefs),
    );
  }

  Widget _buildBody(MailProvider mail, SwipePreferences prefs) {
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

    // Added: TMAIL-148 — compute which directions are actually wired so we can
    // pass the right DismissDirection to Dismissible. Picking "None" on a side
    // disables swiping in that direction entirely.
    final rightEnabled = prefs.rightAction != SwipeAction.none;
    final leftEnabled = prefs.leftAction != SwipeAction.none;
    final DismissDirection direction;
    if (rightEnabled && leftEnabled) {
      direction = DismissDirection.horizontal;
    } else if (rightEnabled) {
      direction = DismissDirection.startToEnd;
    } else if (leftEnabled) {
      direction = DismissDirection.endToStart;
    } else {
      direction = DismissDirection.none;
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
            direction: direction,
            background: Container(
              key: const Key('swipe_right_bg'),
              color: _backgroundColor(prefs.rightAction),
              alignment: Alignment.centerLeft,
              padding: const EdgeInsets.only(left: 16),
              child: Icon(_backgroundIcon(prefs.rightAction),
                  color: Colors.white),
            ),
            secondaryBackground: Container(
              key: const Key('swipe_left_bg'),
              color: _backgroundColor(prefs.leftAction),
              alignment: Alignment.centerRight,
              padding: const EdgeInsets.only(right: 16),
              child: Icon(_backgroundIcon(prefs.leftAction),
                  color: Colors.white),
            ),
            confirmDismiss: (swipeDirection) {
              final action = swipeDirection == DismissDirection.startToEnd
                  ? prefs.rightAction
                  : prefs.leftAction;
              return _performAction(action, message);
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
