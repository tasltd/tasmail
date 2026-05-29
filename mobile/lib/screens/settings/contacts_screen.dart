// Added: Contacts list screen for TMAIL-152
// PURPOSE: Read-only browse + search of the user's contact book on mobile.
//          Editing lives in the SPA today; the mobile screen focuses on the
//          most common need — "who am I emailing?" lookup.
// EXTERNAL: SettingsApi -> backend handlers/contacts.rs (GET /api/contacts).

import 'dart:async';

import 'package:flutter/material.dart';
import '../../api/settings_api.dart';

class ContactsScreen extends StatefulWidget {
  final SettingsApi? api;
  const ContactsScreen({super.key, this.api});

  @override
  State<ContactsScreen> createState() => _ContactsScreenState();
}

class _ContactsScreenState extends State<ContactsScreen> {
  late final SettingsApi _api = widget.api ?? SettingsApi();
  final TextEditingController _searchCtrl = TextEditingController();
  Timer? _debounce;
  List<ContactRecord> _items = const [];
  bool _loading = true;
  String? _error;

  @override
  void initState() {
    super.initState();
    _refresh();
  }

  @override
  void dispose() {
    _debounce?.cancel();
    _searchCtrl.dispose();
    super.dispose();
  }

  Future<void> _refresh({String? query}) async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final items = await _api.listContacts(query: query);
      if (!mounted) return;
      setState(() {
        _items = items;
        _loading = false;
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _error = describeError(e);
        _loading = false;
      });
    }
  }

  void _onSearchChanged(String value) {
    // NOTE: Debounce to avoid hammering /api/contacts on every keystroke —
    //       300 ms feels responsive without spamming the backend.
    _debounce?.cancel();
    _debounce = Timer(const Duration(milliseconds: 300), () {
      _refresh(query: value.trim().isEmpty ? null : value.trim());
    });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Contacts'),
        bottom: PreferredSize(
          preferredSize: const Size.fromHeight(56),
          child: Padding(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 8),
            child: TextField(
              controller: _searchCtrl,
              onChanged: _onSearchChanged,
              decoration: InputDecoration(
                hintText: 'Search by name, email, company',
                prefixIcon: const Icon(Icons.search),
                suffixIcon: _searchCtrl.text.isEmpty
                    ? null
                    : IconButton(
                        icon: const Icon(Icons.clear),
                        onPressed: () {
                          _searchCtrl.clear();
                          _onSearchChanged('');
                        },
                      ),
                filled: true,
                border: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(24),
                  borderSide: BorderSide.none,
                ),
                contentPadding: const EdgeInsets.symmetric(horizontal: 16),
              ),
            ),
          ),
        ),
      ),
      body: _buildBody(),
    );
  }

  Widget _buildBody() {
    if (_loading) {
      return const Center(child: CircularProgressIndicator());
    }
    if (_error != null) {
      return _ErrorView(message: _error!, onRetry: () => _refresh());
    }
    if (_items.isEmpty) {
      return const _EmptyView(
        icon: Icons.contacts_outlined,
        title: 'No contacts',
        subtitle:
            'Contacts you email auto-populate here. Add new contacts from the web app.',
      );
    }
    return RefreshIndicator(
      onRefresh: () => _refresh(query: _searchCtrl.text.trim()),
      child: ListView.separated(
        itemCount: _items.length,
        separatorBuilder: (_, __) => const Divider(height: 1),
        itemBuilder: (_, i) {
          final c = _items[i];
          final display = c.displayName ?? c.email;
          return ListTile(
            leading: CircleAvatar(
              child: Text(display.substring(0, 1).toUpperCase()),
            ),
            title: Text(display),
            subtitle: Text(c.email),
            trailing: c.company != null && c.company!.isNotEmpty
                ? Text(
                    c.company!,
                    style:
                        TextStyle(color: Theme.of(context).hintColor),
                  )
                : null,
            onTap: () => _showDetails(c),
          );
        },
      ),
    );
  }

  void _showDetails(ContactRecord c) {
    showModalBottomSheet(
      context: context,
      isScrollControlled: true,
      builder: (_) => Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(c.displayName ?? c.email,
                style: Theme.of(context).textTheme.titleLarge),
            const SizedBox(height: 12),
            _row(Icons.email_outlined, c.email),
            if (c.company != null && c.company!.isNotEmpty)
              _row(Icons.business, c.company!),
            if (c.phone != null && c.phone!.isNotEmpty)
              _row(Icons.phone, c.phone!),
            const SizedBox(height: 20),
          ],
        ),
      ),
    );
  }

  Widget _row(IconData icon, String text) => Padding(
        padding: const EdgeInsets.symmetric(vertical: 6),
        child: Row(
          children: [
            Icon(icon, size: 20, color: Theme.of(context).hintColor),
            const SizedBox(width: 12),
            Expanded(child: Text(text)),
          ],
        ),
      );
}

class _EmptyView extends StatelessWidget {
  final IconData icon;
  final String title;
  final String subtitle;
  const _EmptyView({
    required this.icon,
    required this.title,
    required this.subtitle,
  });

  @override
  Widget build(BuildContext context) {
    final cs = Theme.of(context).colorScheme;
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(icon, size: 48, color: cs.onSurfaceVariant),
            const SizedBox(height: 12),
            Text(title, style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 4),
            Text(
              subtitle,
              textAlign: TextAlign.center,
              style: TextStyle(color: cs.onSurfaceVariant),
            ),
          ],
        ),
      ),
    );
  }
}

class _ErrorView extends StatelessWidget {
  final String message;
  final VoidCallback onRetry;
  const _ErrorView({required this.message, required this.onRetry});

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            const Icon(Icons.error_outline,
                size: 40, color: Colors.redAccent),
            const SizedBox(height: 12),
            Text(message, textAlign: TextAlign.center),
            const SizedBox(height: 12),
            OutlinedButton(onPressed: onRetry, child: const Text('Retry')),
          ],
        ),
      ),
    );
  }
}
