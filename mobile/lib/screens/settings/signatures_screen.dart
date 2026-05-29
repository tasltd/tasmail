// Added: Signatures CRUD screen for TMAIL-152
// PURPOSE: Let the mobile user list, create, edit, delete and mark-default
//          their email signatures. Hits the same REST endpoints as the SPA
//          (frontend/src/components/settings/SignatureManager.tsx).
// EXTERNAL: SettingsApi -> backend handlers/signatures.rs.

import 'package:flutter/material.dart';
import '../../api/settings_api.dart';

class SignaturesScreen extends StatefulWidget {
  final SettingsApi? api;
  const SignaturesScreen({super.key, this.api});

  @override
  State<SignaturesScreen> createState() => _SignaturesScreenState();
}

class _SignaturesScreenState extends State<SignaturesScreen> {
  late final SettingsApi _api = widget.api ?? SettingsApi();
  List<SignatureRecord> _items = const [];
  bool _loading = true;
  String? _error;

  @override
  void initState() {
    super.initState();
    _refresh();
  }

  Future<void> _refresh() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final items = await _api.listSignatures();
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

  Future<void> _openEditor({SignatureRecord? existing}) async {
    final saved = await Navigator.of(context).push<bool>(
      MaterialPageRoute(
        builder: (_) => _SignatureEditor(api: _api, existing: existing),
      ),
    );
    if (saved == true) await _refresh();
  }

  Future<void> _delete(SignatureRecord sig) async {
    final ok = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Delete signature?'),
        content: Text('"${sig.name}" will be removed permanently.'),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(ctx, false),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () => Navigator.pop(ctx, true),
            child: const Text('Delete'),
          ),
        ],
      ),
    );
    if (ok != true) return;
    try {
      await _api.deleteSignature(sig.id);
      await _refresh();
    } catch (e) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(describeError(e))),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Signatures')),
      floatingActionButton: FloatingActionButton(
        onPressed: () => _openEditor(),
        tooltip: 'New signature',
        child: const Icon(Icons.add),
      ),
      body: _buildBody(),
    );
  }

  Widget _buildBody() {
    if (_loading) {
      return const Center(child: CircularProgressIndicator());
    }
    if (_error != null) {
      return _ErrorView(message: _error!, onRetry: _refresh);
    }
    if (_items.isEmpty) {
      return const _EmptyView(
        icon: Icons.draw_outlined,
        title: 'No signatures yet',
        subtitle: 'Tap + to add one you can append to outgoing mail.',
      );
    }
    return RefreshIndicator(
      onRefresh: _refresh,
      child: ListView.separated(
        itemCount: _items.length,
        separatorBuilder: (_, __) => const Divider(height: 1),
        itemBuilder: (_, i) {
          final sig = _items[i];
          return ListTile(
            leading: const Icon(Icons.draw),
            title: Text(sig.name),
            subtitle: Text(
              sig.textBody.isNotEmpty ? sig.textBody : '(no plain text)',
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
            ),
            trailing: Wrap(
              spacing: 4,
              children: [
                if (sig.isDefault)
                  const Chip(
                    label: Text('Default'),
                    visualDensity: VisualDensity.compact,
                  ),
                IconButton(
                  icon: const Icon(Icons.delete_outline),
                  onPressed: () => _delete(sig),
                  tooltip: 'Delete',
                ),
              ],
            ),
            onTap: () => _openEditor(existing: sig),
          );
        },
      ),
    );
  }
}

class _SignatureEditor extends StatefulWidget {
  final SettingsApi api;
  final SignatureRecord? existing;
  const _SignatureEditor({required this.api, this.existing});

  @override
  State<_SignatureEditor> createState() => _SignatureEditorState();
}

class _SignatureEditorState extends State<_SignatureEditor> {
  late final TextEditingController _name =
      TextEditingController(text: widget.existing?.name ?? '');
  late final TextEditingController _text =
      TextEditingController(text: widget.existing?.textBody ?? '');
  late bool _isDefault = widget.existing?.isDefault ?? false;
  bool _saving = false;
  String? _error;

  @override
  void dispose() {
    _name.dispose();
    _text.dispose();
    super.dispose();
  }

  Future<void> _save() async {
    if (_name.text.trim().isEmpty) {
      setState(() => _error = 'Name is required');
      return;
    }
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      if (widget.existing == null) {
        await widget.api.createSignature(
          name: _name.text.trim(),
          textBody: _text.text,
          htmlBody: _text.text,
          isDefault: _isDefault,
        );
      } else {
        await widget.api.updateSignature(
          id: widget.existing!.id,
          name: _name.text.trim(),
          textBody: _text.text,
          htmlBody: _text.text,
          isDefault: _isDefault,
        );
      }
      if (!mounted) return;
      Navigator.pop(context, true);
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _error = describeError(e);
        _saving = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text(widget.existing == null ? 'New Signature' : 'Edit Signature'),
        actions: [
          TextButton(
            onPressed: _saving ? null : _save,
            child: Text(_saving ? 'Saving...' : 'Save'),
          ),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          TextField(
            controller: _name,
            decoration: const InputDecoration(
              labelText: 'Name',
              helperText: 'Shown in the picker — not sent to recipients.',
            ),
          ),
          const SizedBox(height: 16),
          TextField(
            controller: _text,
            decoration: const InputDecoration(
              labelText: 'Signature text',
              alignLabelWithHint: true,
              border: OutlineInputBorder(),
            ),
            maxLines: 8,
            minLines: 4,
          ),
          const SizedBox(height: 8),
          SwitchListTile(
            title: const Text('Set as default'),
            subtitle: const Text('Append this to new emails automatically.'),
            value: _isDefault,
            onChanged: (v) => setState(() => _isDefault = v),
          ),
          if (_error != null)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Text(_error!,
                  style: const TextStyle(color: Colors.redAccent)),
            ),
        ],
      ),
    );
  }
}

// Internal small reusable widgets — kept private so the file stays self-contained.

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
