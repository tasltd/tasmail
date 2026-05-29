// Added: 2FA (TOTP) setup screen for TMAIL-152
// PURPOSE: Walk the user through TOTP enrolment — fetch a secret + otpauth URL
//          from POST /api/2fa/enroll, let them paste the secret into their
//          authenticator app, then verify with a 6-digit code. Also handles
//          status display and disabling.
// NOTE: We intentionally display the otpauth URL + secret as plain text
//       instead of rendering a QR. qr_flutter isn't wired into pubspec yet,
//       and most authenticator apps accept "Enter setup key" as a fallback.
//       Adding QR rendering is a follow-up — see docs/MOBILE-FCM-SETUP.md
//       neighbours for the dependency-add pattern.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../../api/settings_api.dart';

class TwoFactorScreen extends StatefulWidget {
  final SettingsApi? api;
  const TwoFactorScreen({super.key, this.api});

  @override
  State<TwoFactorScreen> createState() => _TwoFactorScreenState();
}

class _TwoFactorScreenState extends State<TwoFactorScreen> {
  late final SettingsApi _api = widget.api ?? SettingsApi();
  TwoFactorStatus? _status;
  TwoFactorEnrollment? _enrollment;
  bool _loading = true;
  String? _error;

  final _codeCtrl = TextEditingController();
  bool _verifying = false;
  String? _verifyError;

  @override
  void initState() {
    super.initState();
    _loadStatus();
  }

  @override
  void dispose() {
    _codeCtrl.dispose();
    super.dispose();
  }

  Future<void> _loadStatus() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final status = await _api.twoFactorStatus();
      if (!mounted) return;
      setState(() {
        _status = status;
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

  Future<void> _startEnroll() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final enrollment = await _api.twoFactorEnroll();
      if (!mounted) return;
      setState(() {
        _enrollment = enrollment;
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

  Future<void> _verify() async {
    final code = _codeCtrl.text.trim();
    if (code.length < 6) {
      setState(() => _verifyError = 'Enter the 6-digit code from your app');
      return;
    }
    setState(() {
      _verifying = true;
      _verifyError = null;
    });
    try {
      await _api.twoFactorVerify(code);
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Two-factor authentication enabled')),
      );
      setState(() {
        _enrollment = null;
        _codeCtrl.clear();
        _verifying = false;
      });
      await _loadStatus();
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _verifyError = describeError(e);
        _verifying = false;
      });
    }
  }

  Future<void> _disable() async {
    final ok = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Disable 2FA?'),
        content: const Text(
          'Your account will no longer require a second factor at login.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(ctx, false),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () => Navigator.pop(ctx, true),
            child: const Text('Disable'),
          ),
        ],
      ),
    );
    if (ok != true) return;
    try {
      await _api.twoFactorDisable();
      await _loadStatus();
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Two-factor authentication disabled')),
      );
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
      appBar: AppBar(title: const Text('Two-Factor Authentication')),
      body: _buildBody(),
    );
  }

  Widget _buildBody() {
    if (_loading) {
      return const Center(child: CircularProgressIndicator());
    }
    if (_error != null) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              const Icon(Icons.error_outline,
                  size: 40, color: Colors.redAccent),
              const SizedBox(height: 12),
              Text(_error!, textAlign: TextAlign.center),
              const SizedBox(height: 12),
              OutlinedButton(
                onPressed: _loadStatus,
                child: const Text('Retry'),
              ),
            ],
          ),
        ),
      );
    }
    if (_enrollment != null) {
      return _buildEnrollmentFlow(_enrollment!);
    }
    return _buildStatusView();
  }

  Widget _buildStatusView() {
    final enabled = _status?.enabled ?? false;
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        Card(
          child: ListTile(
            leading: Icon(
              enabled ? Icons.verified_user : Icons.lock_outline,
              color: enabled ? Colors.green : null,
            ),
            title: Text(enabled ? '2FA is enabled' : '2FA is disabled'),
            subtitle: enabled
                ? Text(
                    'Backup codes remaining: '
                    '${_status?.backupCodesRemaining ?? 0}',
                  )
                : const Text(
                    'Add a second factor to make account takeover much harder.',
                  ),
          ),
        ),
        const SizedBox(height: 16),
        if (!enabled)
          FilledButton.icon(
            icon: const Icon(Icons.qr_code_2),
            label: const Text('Set up authenticator app'),
            onPressed: _startEnroll,
          )
        else
          OutlinedButton.icon(
            icon: const Icon(Icons.logout),
            label: const Text('Disable 2FA'),
            style: OutlinedButton.styleFrom(foregroundColor: Colors.redAccent),
            onPressed: _disable,
          ),
        const SizedBox(height: 24),
        const Text(
          'How it works',
          style: TextStyle(fontWeight: FontWeight.bold),
        ),
        const SizedBox(height: 8),
        const Text(
          '1. Install an authenticator app (Google Authenticator, Authy, 1Password).\n'
          '2. Add an account using the setup key we generate.\n'
          '3. Enter the 6-digit code shown by the app to finish setup.',
        ),
      ],
    );
  }

  Widget _buildEnrollmentFlow(TwoFactorEnrollment e) {
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        const Text(
          'Step 1 — Add to your authenticator app',
          style: TextStyle(fontWeight: FontWeight.bold),
        ),
        const SizedBox(height: 8),
        const Text(
          'Open your authenticator app, choose "Enter setup key" '
          '(or scan if the app supports it), then paste the secret below.',
        ),
        const SizedBox(height: 16),
        _CopyableField(label: 'Setup key', value: e.secret),
        const SizedBox(height: 12),
        _CopyableField(label: 'otpauth:// URL', value: e.otpauthUrl),
        const SizedBox(height: 24),
        const Text(
          'Step 2 — Verify the code',
          style: TextStyle(fontWeight: FontWeight.bold),
        ),
        const SizedBox(height: 8),
        TextField(
          controller: _codeCtrl,
          keyboardType: TextInputType.number,
          maxLength: 6,
          decoration: InputDecoration(
            labelText: '6-digit code',
            errorText: _verifyError,
            border: const OutlineInputBorder(),
          ),
        ),
        const SizedBox(height: 8),
        FilledButton(
          onPressed: _verifying ? null : _verify,
          child: Text(_verifying ? 'Verifying...' : 'Verify and enable'),
        ),
        const SizedBox(height: 24),
        const Text(
          'Backup codes — store these somewhere safe',
          style: TextStyle(fontWeight: FontWeight.bold),
        ),
        const SizedBox(height: 8),
        const Text(
          'Each code works once. Use one if you ever lose access to your '
          'authenticator app.',
        ),
        const SizedBox(height: 8),
        ...e.backupCodes.map(
          (c) => SelectableText(
            c,
            style: const TextStyle(fontFamily: 'monospace', fontSize: 16),
          ),
        ),
        const SizedBox(height: 16),
        TextButton(
          onPressed: () => setState(() {
            _enrollment = null;
            _codeCtrl.clear();
            _verifyError = null;
          }),
          child: const Text('Cancel setup'),
        ),
      ],
    );
  }
}

class _CopyableField extends StatelessWidget {
  final String label;
  final String value;
  const _CopyableField({required this.label, required this.value});

  @override
  Widget build(BuildContext context) {
    return InputDecorator(
      decoration: InputDecoration(
        labelText: label,
        border: const OutlineInputBorder(),
        suffixIcon: IconButton(
          icon: const Icon(Icons.copy),
          tooltip: 'Copy',
          onPressed: () {
            Clipboard.setData(ClipboardData(text: value));
            ScaffoldMessenger.of(context).showSnackBar(
              SnackBar(content: Text('$label copied')),
            );
          },
        ),
      ),
      child: SelectableText(
        value,
        style: const TextStyle(fontFamily: 'monospace'),
      ),
    );
  }
}
