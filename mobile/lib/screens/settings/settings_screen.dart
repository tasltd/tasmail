// Added: Settings hub screen for TMAIL-152
// PURPOSE: Central settings page with navigation to sub-settings (account, signatures, contacts, 2FA, etc.)
// EXTERNAL: Uses AuthProvider for user info

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import '../../providers/auth_provider.dart';

class SettingsScreen extends StatelessWidget {
  const SettingsScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final auth = context.watch<AuthProvider>();
    final theme = Theme.of(context);

    return Scaffold(
      appBar: AppBar(title: const Text('Settings')),
      body: ListView(
        children: [
          // Added: Account info card
          Card(
            margin: const EdgeInsets.all(16),
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Row(
                children: [
                  CircleAvatar(
                    radius: 28,
                    backgroundColor: theme.colorScheme.primaryContainer,
                    child: Text(
                      (auth.user?.email ?? '?').substring(0, 1).toUpperCase(),
                      style: TextStyle(
                        fontSize: 20,
                        color: theme.colorScheme.onPrimaryContainer,
                      ),
                    ),
                  ),
                  const SizedBox(width: 16),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          auth.user?.displayName ?? 'User',
                          style: theme.textTheme.titleMedium,
                        ),
                        Text(
                          auth.user?.email ?? '',
                          style: TextStyle(
                            color: theme.colorScheme.onSurfaceVariant,
                          ),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),

          // Added: Settings sections
          _buildSection(context, 'Mail', [
            _SettingItem(Icons.draw, 'Signatures', '/settings/signatures'),
            _SettingItem(Icons.contacts, 'Contacts', '/settings/contacts'),
            _SettingItem(Icons.label, 'Labels & Folders', '/settings/folders'),
            _SettingItem(Icons.filter_list, 'Filters', '/settings/filters'),
            _SettingItem(Icons.reply_all, 'Auto Reply', '/settings/auto-reply'),
            // Added: TMAIL-148 — configurable inbox swipe actions
            _SettingItem(
              Icons.swipe,
              'Swipe Actions',
              '/settings/swipe-actions',
            ),
          ]),

          _buildSection(context, 'Security', [
            _SettingItem(Icons.security, 'Two-Factor Auth', '/settings/2fa'),
            _SettingItem(Icons.fingerprint, 'Biometric Lock', '/settings/biometric'),
            _SettingItem(Icons.devices, 'Active Sessions', '/settings/sessions'),
          ]),

          _buildSection(context, 'Notifications', [
            _SettingItem(Icons.notifications, 'Push Notifications', '/settings/notifications'),
          ]),

          _buildSection(context, 'Storage', [
            _SettingItem(Icons.storage, 'Quota', '/settings/quota'),
            _SettingItem(Icons.attach_file, 'Attachments', '/settings/attachments'),
          ]),

          _buildSection(context, 'About', [
            _SettingItem(Icons.info, 'About TASMail', '/settings/about'),
          ]),

          const SizedBox(height: 16),

          // Added: Logout button
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            child: OutlinedButton.icon(
              onPressed: () => auth.logout(),
              icon: const Icon(Icons.logout),
              label: const Text('Sign Out'),
              style: OutlinedButton.styleFrom(
                foregroundColor: theme.colorScheme.error,
              ),
            ),
          ),

          const SizedBox(height: 32),
        ],
      ),
    );
  }

  Widget _buildSection(
    BuildContext context,
    String title,
    List<_SettingItem> items,
  ) {
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 16, 16, 4),
          child: Text(
            title,
            style: TextStyle(
              fontSize: 13,
              fontWeight: FontWeight.w600,
              color: theme.colorScheme.primary,
            ),
          ),
        ),
        ...items.map((item) => ListTile(
              leading: Icon(item.icon),
              title: Text(item.label),
              trailing: const Icon(Icons.chevron_right, size: 20),
              onTap: () {
                // Changed: Wired Biometric Lock (TMAIL-142) and Swipe Actions
                // (TMAIL-148) routes; other sub-screens still show the
                // "coming soon" placeholder until they ship.
                if (item.route == '/settings/biometric' ||
                    item.route == '/settings/swipe-actions') {
                  Navigator.pushNamed(context, item.route);
                  return;
                }
                ScaffoldMessenger.of(context).showSnackBar(
                  SnackBar(content: Text('${item.label} - coming soon')),
                );
              },
            )),
      ],
    );
  }
}

class _SettingItem {
  final IconData icon;
  final String label;
  final String route;
  const _SettingItem(this.icon, this.label, this.route);
}
