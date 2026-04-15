// Added: Folder navigation drawer for TMAIL-146
// PURPOSE: Side drawer showing folder tree with unread counts and folder switching
// EXTERNAL: Uses MailProvider for folder data

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../../providers/auth_provider.dart';
import '../../providers/mail_provider.dart';

class FolderDrawer extends StatelessWidget {
  const FolderDrawer({super.key});

  // Added: Map folder names to Material icons
  static IconData _folderIcon(String name) {
    switch (name.toUpperCase()) {
      case 'INBOX':
        return Icons.inbox;
      case 'SENT':
      case 'SENT ITEMS':
        return Icons.send;
      case 'DRAFTS':
        return Icons.drafts;
      case 'TRASH':
      case 'DELETED ITEMS':
        return Icons.delete;
      case 'SPAM':
      case 'JUNK':
        return Icons.report;
      case 'ARCHIVE':
        return Icons.archive;
      default:
        return Icons.folder;
    }
  }

  @override
  Widget build(BuildContext context) {
    final mail = context.watch<MailProvider>();
    final auth = context.watch<AuthProvider>();
    final theme = Theme.of(context);

    return Drawer(
      child: Column(
        children: [
          // Added: User info header
          UserAccountsDrawerHeader(
            accountName: Text(auth.user?.displayName ?? auth.user?.email ?? ''),
            accountEmail: Text(auth.user?.email ?? ''),
            currentAccountPicture: CircleAvatar(
              backgroundColor: theme.colorScheme.primaryContainer,
              child: Text(
                (auth.user?.email ?? '?').substring(0, 1).toUpperCase(),
                style: TextStyle(
                  fontSize: 24,
                  color: theme.colorScheme.onPrimaryContainer,
                ),
              ),
            ),
          ),

          // Added: Folder list
          Expanded(
            child: mail.isLoadingFolders && mail.folders.isEmpty
                ? const Center(child: CircularProgressIndicator())
                : ListView.builder(
                    itemCount: mail.folders.length,
                    itemBuilder: (context, index) {
                      final folder = mail.folders[index];
                      final isSelected = folder.name == mail.selectedFolder;

                      return ListTile(
                        leading: Icon(
                          _folderIcon(folder.name),
                          color: isSelected
                              ? theme.colorScheme.primary
                              : theme.colorScheme.onSurfaceVariant,
                        ),
                        title: Text(
                          folder.name,
                          style: TextStyle(
                            fontWeight:
                                isSelected ? FontWeight.bold : FontWeight.normal,
                          ),
                        ),
                        trailing: folder.unreadCount > 0
                            ? Badge(
                                label: Text('${folder.unreadCount}'),
                              )
                            : null,
                        selected: isSelected,
                        onTap: () {
                          mail.selectFolder(folder.name);
                          Navigator.pop(context);
                        },
                      );
                    },
                  ),
          ),

          const Divider(),

          // Added: Settings and logout
          ListTile(
            leading: const Icon(Icons.settings),
            title: const Text('Settings'),
            onTap: () {
              Navigator.pop(context);
              Navigator.pushNamed(context, '/settings');
            },
          ),
          ListTile(
            leading: const Icon(Icons.logout),
            title: const Text('Sign Out'),
            onTap: () async {
              await auth.logout();
            },
          ),
          const SizedBox(height: 8),
        ],
      ),
    );
  }
}
