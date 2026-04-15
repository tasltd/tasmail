// Added: Reusable message list tile widget for TMAIL-143
// PURPOSE: Displays a single email summary with read/unread styling, flag, attachment indicator

import 'package:flutter/material.dart';
import '../models/email.dart';

class MessageTile extends StatelessWidget {
  final MobileMessageSummary message;
  final VoidCallback onTap;
  final VoidCallback? onFlagToggle;

  const MessageTile({
    super.key,
    required this.message,
    required this.onTap,
    this.onFlagToggle,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final isUnread = !message.isRead;

    return ListTile(
      onTap: onTap,
      contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
      leading: CircleAvatar(
        backgroundColor: isUnread
            ? theme.colorScheme.primary
            : theme.colorScheme.surfaceContainerHighest,
        child: Text(
          (message.from ?? '?').isNotEmpty
              ? (message.from!).substring(0, 1).toUpperCase()
              : '?',
          style: TextStyle(
            color: isUnread
                ? theme.colorScheme.onPrimary
                : theme.colorScheme.onSurfaceVariant,
          ),
        ),
      ),
      title: Text(
        message.from ?? 'Unknown',
        style: TextStyle(
          fontWeight: isUnread ? FontWeight.bold : FontWeight.normal,
          fontSize: 14,
        ),
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
      ),
      subtitle: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            message.subject ?? '(no subject)',
            style: TextStyle(
              fontWeight: isUnread ? FontWeight.w600 : FontWeight.normal,
              fontSize: 13,
              color: theme.colorScheme.onSurface,
            ),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
          ),
          const SizedBox(height: 2),
          Row(
            children: [
              Text(
                _formatDate(message.date),
                style: TextStyle(
                  fontSize: 11,
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
              if (message.hasAttachment) ...[
                const SizedBox(width: 6),
                Icon(
                  Icons.attach_file,
                  size: 14,
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ],
            ],
          ),
        ],
      ),
      trailing: IconButton(
        icon: Icon(
          message.isFlagged ? Icons.star : Icons.star_border,
          color: message.isFlagged ? Colors.amber : theme.colorScheme.onSurfaceVariant,
          size: 20,
        ),
        onPressed: onFlagToggle,
      ),
    );
  }

  // Added: Format date string for display
  String _formatDate(String? dateStr) {
    if (dateStr == null) return '';
    try {
      final date = DateTime.parse(dateStr);
      final now = DateTime.now();
      final diff = now.difference(date);

      if (diff.inDays == 0) {
        return '${date.hour.toString().padLeft(2, '0')}:${date.minute.toString().padLeft(2, '0')}';
      } else if (diff.inDays < 7) {
        const days = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
        return days[date.weekday - 1];
      } else {
        return '${date.day}/${date.month}/${date.year}';
      }
    } catch (_) {
      return dateStr;
    }
  }
}
