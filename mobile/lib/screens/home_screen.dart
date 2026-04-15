// Added: Home screen with bottom navigation for TMAIL-149
// PURPOSE: Main app shell with bottom nav bar (Inbox, Search, Settings) and folder drawer
// EXTERNAL: Integrates InboxScreen, SearchScreen, SettingsScreen, FolderDrawer

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../providers/mail_provider.dart';
import 'inbox/inbox_screen.dart';
import 'search/search_screen.dart';
import 'settings/settings_screen.dart';
import 'folders/folder_drawer.dart';

class HomeScreen extends StatefulWidget {
  const HomeScreen({super.key});

  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends State<HomeScreen> {
  int _currentIndex = 0;

  // Added: Screens for bottom nav tabs
  final List<Widget> _screens = const [
    InboxScreen(),
    SearchScreen(),
    SettingsScreen(),
  ];

  @override
  void initState() {
    super.initState();
    // Added: Load folders on home screen init
    WidgetsBinding.instance.addPostFrameCallback((_) {
      context.read<MailProvider>().loadFolders();
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
            icon: Icon(Icons.settings),
            label: 'Settings',
          ),
        ],
      ),
      // Added: FAB for compose on inbox tab
      floatingActionButton: _currentIndex == 0
          ? FloatingActionButton(
              key: const Key('compose_fab'),
              onPressed: () => Navigator.pushNamed(context, '/compose'),
              child: const Icon(Icons.edit),
            )
          : null,
    );
  }
}
