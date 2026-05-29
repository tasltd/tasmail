// Added: Home screen with bottom navigation for TMAIL-149
// Changed: TMAIL-149 — added the Calendar destination so the bottom nav now
//          matches the spec (Inbox, Search, Calendar, Settings + Compose FAB),
//          and the Compose FAB is shown across all tabs because compose is a
//          global action (the issue lists it as a peer of the destinations).
//          Also kicks off MailProvider.loadUnreadCount() on mount so the Inbox
//          badge actually populates from /api/mobile/unread-count instead of
//          staying at 0 until something else triggers a load.
// PURPOSE: Main app shell with bottom nav bar (Inbox, Search, Calendar, Settings)
//          plus a Compose FAB and the folder drawer.
// EXTERNAL: Integrates InboxScreen, SearchScreen, CalendarScreen, SettingsScreen,
//          FolderDrawer. Reads totalUnreadCount from MailProvider.

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../providers/mail_provider.dart';
import 'inbox/inbox_screen.dart';
import 'search/search_screen.dart';
import 'calendar/calendar_screen.dart';
import 'settings/settings_screen.dart';
import 'folders/folder_drawer.dart';

class HomeScreen extends StatefulWidget {
  const HomeScreen({super.key});

  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends State<HomeScreen> {
  int _currentIndex = 0;

  // Added: Screens for bottom nav tabs. Order MUST match the destinations
  //        list below — IndexedStack indexes by position.
  final List<Widget> _screens = const [
    InboxScreen(),
    SearchScreen(),
    CalendarScreen(),
    SettingsScreen(),
  ];

  @override
  void initState() {
    super.initState();
    // Added: Load folders + unread count on home init. loadUnreadCount drives
    //        the Inbox badge; without it the badge sits at 0 on cold start
    //        even though /api/mobile/unread-count is available.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      final mail = context.read<MailProvider>();
      mail.loadFolders();
      mail.loadUnreadCount();
    });
  }

  @override
  Widget build(BuildContext context) {
    final mail = context.watch<MailProvider>();

    return Scaffold(
      drawer: const FolderDrawer(),
      body: IndexedStack(
        index: _currentIndex,
        children: _screens,
      ),
      bottomNavigationBar: NavigationBar(
        selectedIndex: _currentIndex,
        onDestinationSelected: (index) {
          setState(() => _currentIndex = index);
        },
        destinations: [
          NavigationDestination(
            icon: Badge(
              isLabelVisible: mail.totalUnreadCount > 0,
              label: Text('${mail.totalUnreadCount}'),
              child: const Icon(Icons.inbox),
            ),
            label: 'Inbox',
          ),
          const NavigationDestination(
            icon: Icon(Icons.search),
            label: 'Search',
          ),
          const NavigationDestination(
            icon: Icon(Icons.calendar_month),
            label: 'Calendar',
          ),
          const NavigationDestination(
            icon: Icon(Icons.settings),
            label: 'Settings',
          ),
        ],
      ),
      // Changed: TMAIL-149 — compose is a global action (listed in the spec as
      //          a peer of the destinations), so the FAB stays visible on all
      //          tabs instead of just the Inbox tab.
      floatingActionButton: FloatingActionButton(
        key: const Key('compose_fab'),
        tooltip: 'Compose',
        onPressed: () => Navigator.pushNamed(context, '/compose'),
        child: const Icon(Icons.edit),
      ),
    );
  }
}
