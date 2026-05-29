// Added: Calendar tab placeholder for TMAIL-149
// PURPOSE: Backs the Calendar destination in the bottom navigation bar so the
//          tab is reachable from the IndexedStack home shell. Full calendar
//          UX (event list, /api/calendar/events binding, ICS RSVP) is tracked
//          in follow-up mobile-calendar issues; this screen just lands users
//          on a coherent "coming soon" surface instead of a blank container.
// EXTERNAL: None. Pure presentation.

import 'package:flutter/material.dart';

class CalendarScreen extends StatelessWidget {
  const CalendarScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Scaffold(
      appBar: AppBar(title: const Text('Calendar')),
      body: Center(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Icon(
                Icons.calendar_month_outlined,
                size: 72,
                color: theme.colorScheme.primary,
              ),
              const SizedBox(height: 16),
              Text(
                'Calendar coming soon',
                style: theme.textTheme.titleLarge,
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 8),
              Text(
                'View events and meeting invites from your inbox here. '
                'Open invites land in the Inbox tab today and will surface '
                'on this calendar in a follow-up release.',
                style: theme.textTheme.bodyMedium,
                textAlign: TextAlign.center,
              ),
            ],
          ),
        ),
      ),
    );
  }
}
