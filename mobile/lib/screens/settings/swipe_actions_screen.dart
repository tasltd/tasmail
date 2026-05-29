// Added: Settings UI for configuring inbox swipe actions (TMAIL-148)
// PURPOSE: Lets the user rebind swipe-right (leading) and swipe-left (trailing)
//          to any supported SwipeAction. Reads/writes through SwipeActionsService
//          so the InboxScreen reacts via ChangeNotifier.
// EXTERNAL: SwipeActionsService + SwipePreferences from services/.
// NOTE: Service is injected via constructor so widget tests can swap in a
//       fake without touching flutter_secure_storage. Production code should
//       construct the service once at app startup and pass the same instance
//       to both this screen and the InboxScreen.

import 'package:flutter/material.dart';

import '../../services/swipe_actions_service.dart';
import '../../services/swipe_preferences.dart';

class SwipeActionsScreen extends StatefulWidget {
  final SwipeActionsService service;

  const SwipeActionsScreen({super.key, required this.service});

  @override
  State<SwipeActionsScreen> createState() => _SwipeActionsScreenState();
}

class _SwipeActionsScreenState extends State<SwipeActionsScreen> {
  bool _busy = false;

  @override
  void initState() {
    super.initState();
    widget.service.addListener(_onPrefsChanged);
    // PURPOSE: Ensure the persisted value is loaded before we render the
    //          pickers. If load() has already run on app startup this is a
    //          fast no-op; otherwise it fetches from secure storage once.
    if (!widget.service.isLoaded) {
      widget.service.load();
    }
  }

  @override
  void dispose() {
    widget.service.removeListener(_onPrefsChanged);
    super.dispose();
  }

  void _onPrefsChanged() {
    if (!mounted) return;
    setState(() {});
  }

  Future<void> _setRight(SwipeAction action) async {
    if (_busy) return;
    setState(() => _busy = true);
    try {
      await widget.service.setRight(action);
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _setLeft(SwipeAction action) async {
    if (_busy) return;
    setState(() => _busy = true);
    try {
      await widget.service.setLeft(action);
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _reset() async {
    if (_busy) return;
    setState(() => _busy = true);
    try {
      await widget.service.reset();
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Swipe actions reset to defaults')),
        );
      }
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final prefs = widget.service.preferences;
    return Scaffold(
      appBar: AppBar(
        title: const Text('Swipe Actions'),
        actions: [
          IconButton(
            key: const Key('swipe_actions_reset'),
            tooltip: 'Restore defaults',
            icon: const Icon(Icons.restart_alt),
            onPressed: _busy ? null : _reset,
          ),
        ],
      ),
      body: ListView(
        children: [
          const Padding(
            padding: EdgeInsets.fromLTRB(16, 16, 16, 8),
            child: Text(
              'Choose what happens when you swipe a message tile in the inbox. '
              'Set either direction to "None" to disable it.',
              style: TextStyle(fontSize: 13),
            ),
          ),
          _SwipeActionPicker(
            key: const Key('swipe_right_picker'),
            title: 'Swipe right',
            subtitle: 'Leading-edge swipe (left → right)',
            icon: Icons.arrow_forward,
            selected: prefs.rightAction,
            onChanged: _setRight,
            enabled: !_busy,
          ),
          const Divider(height: 1),
          _SwipeActionPicker(
            key: const Key('swipe_left_picker'),
            title: 'Swipe left',
            subtitle: 'Trailing-edge swipe (right → left)',
            icon: Icons.arrow_back,
            selected: prefs.leftAction,
            onChanged: _setLeft,
            enabled: !_busy,
          ),
        ],
      ),
    );
  }
}

class _SwipeActionPicker extends StatelessWidget {
  final String title;
  final String subtitle;
  final IconData icon;
  final SwipeAction selected;
  final ValueChanged<SwipeAction> onChanged;
  final bool enabled;

  const _SwipeActionPicker({
    super.key,
    required this.title,
    required this.subtitle,
    required this.icon,
    required this.selected,
    required this.onChanged,
    required this.enabled,
  });

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        ListTile(
          leading: Icon(icon),
          title: Text(title),
          subtitle: Text(subtitle),
        ),
        ...SwipeAction.values.map(
          (action) => RadioListTile<SwipeAction>(
            key: Key('${title.toLowerCase().replaceAll(' ', '_')}_${action.wireName}'),
            value: action,
            groupValue: selected,
            onChanged: enabled
                ? (next) {
                    if (next != null) onChanged(next);
                  }
                : null,
            title: Text(action.displayLabel),
            secondary: Icon(_iconFor(action)),
          ),
        ),
      ],
    );
  }

  IconData _iconFor(SwipeAction action) => switch (action) {
        SwipeAction.none => Icons.block,
        SwipeAction.archive => Icons.archive_outlined,
        SwipeAction.delete => Icons.delete_outline,
        SwipeAction.markUnread => Icons.mark_email_unread_outlined,
        SwipeAction.toggleFlag => Icons.star_outline,
      };
}
