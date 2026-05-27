// Added: Biometric Lock settings screen for TMAIL-142
// PURPOSE: Lets the user configure a fallback PIN, turn biometric unlock on/off,
//          and remove the lock entirely. All persistence is delegated to
//          [BiometricService] — this widget is pure UI + light state.

import 'package:flutter/material.dart';
import 'package:local_auth/local_auth.dart';

import '../../services/biometric_service.dart';

class BiometricSettingsScreen extends StatefulWidget {
  // PURPOSE: Accept an injected service for widget tests; default to a real one.
  final BiometricService? service;

  const BiometricSettingsScreen({super.key, this.service});

  @override
  State<BiometricSettingsScreen> createState() =>
      _BiometricSettingsScreenState();
}

class _BiometricSettingsScreenState extends State<BiometricSettingsScreen> {
  late final BiometricService _service = widget.service ?? BiometricService();

  bool _loading = true;
  bool _biometricEnabled = false;
  bool _hasPin = false;
  bool _deviceSupported = false;
  List<BiometricType> _availableBiometrics = const [];

  @override
  void initState() {
    super.initState();
    _refresh();
  }

  Future<void> _refresh() async {
    final supported = await _service.isDeviceSupported();
    final enabled = await _service.isBiometricEnabled();
    final hasPin = await _service.hasPin();
    final available = supported
        ? await _service.getAvailableBiometrics()
        : const <BiometricType>[];
    if (!mounted) return;
    setState(() {
      _deviceSupported = supported;
      _biometricEnabled = enabled;
      _hasPin = hasPin;
      _availableBiometrics = available;
      _loading = false;
    });
  }

  String _biometricLabel() {
    if (_availableBiometrics.contains(BiometricType.face)) return 'Face ID';
    if (_availableBiometrics.contains(BiometricType.fingerprint)) {
      return 'Fingerprint';
    }
    if (_availableBiometrics.contains(BiometricType.iris)) return 'Iris';
    return 'Biometric';
  }

  Future<void> _onToggleBiometric(bool value) async {
    if (value && !_hasPin) {
      await _showSetPinDialog();
      if (!await _service.hasPin()) return;
    }
    try {
      await _service.setBiometricEnabled(value);
    } on StateError catch (e) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(e.message)),
      );
    }
    await _refresh();
  }

  Future<void> _showSetPinDialog() async {
    final pin = await showDialog<String>(
      context: context,
      builder: (_) => const _PinEntryDialog(
        title: 'Set Fallback PIN',
        confirm: true,
      ),
    );
    if (pin == null) return;
    try {
      await _service.setPin(pin);
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('PIN saved')),
      );
    } on ArgumentError catch (e) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(e.message.toString())),
      );
    }
    await _refresh();
  }

  Future<void> _removeLock() async {
    final confirm = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Remove Lock?'),
        content: const Text(
          'This will disable biometric unlock and erase your fallback PIN.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(ctx, false),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () => Navigator.pop(ctx, true),
            child: const Text('Remove'),
          ),
        ],
      ),
    );
    if (confirm != true) return;
    await _service.clearLock();
    await _refresh();
  }

  @override
  Widget build(BuildContext context) {
    if (_loading) {
      return const Scaffold(body: Center(child: CircularProgressIndicator()));
    }
    return Scaffold(
      appBar: AppBar(title: const Text('Biometric Lock')),
      body: ListView(
        children: [
          if (!_deviceSupported)
            const Padding(
              padding: EdgeInsets.all(16),
              child: Text(
                'This device does not support biometric authentication. '
                'You can still set a PIN to lock the app.',
              ),
            ),
          SwitchListTile(
            secondary: const Icon(Icons.fingerprint),
            title: Text('Unlock with ${_biometricLabel()}'),
            subtitle: Text(
              _deviceSupported
                  ? 'Use ${_biometricLabel().toLowerCase()} when opening the app.'
                  : 'Not available on this device.',
            ),
            value: _biometricEnabled,
            onChanged: _deviceSupported ? _onToggleBiometric : null,
          ),
          ListTile(
            leading: const Icon(Icons.dialpad),
            title: Text(_hasPin ? 'Change Fallback PIN' : 'Set Fallback PIN'),
            subtitle: Text(
              _hasPin
                  ? 'Tap to change your unlock PIN.'
                  : 'Required before turning on biometric unlock.',
            ),
            onTap: _showSetPinDialog,
          ),
          if (_hasPin || _biometricEnabled)
            ListTile(
              leading: const Icon(Icons.lock_open, color: Colors.redAccent),
              title: const Text(
                'Remove Lock',
                style: TextStyle(color: Colors.redAccent),
              ),
              subtitle: const Text('Erase PIN and disable biometric unlock.'),
              onTap: _removeLock,
            ),
        ],
      ),
    );
  }
}

class _PinEntryDialog extends StatefulWidget {
  final String title;
  final bool confirm;
  const _PinEntryDialog({required this.title, this.confirm = false});

  @override
  State<_PinEntryDialog> createState() => _PinEntryDialogState();
}

class _PinEntryDialogState extends State<_PinEntryDialog> {
  final _pinCtrl = TextEditingController();
  final _confirmCtrl = TextEditingController();
  String? _error;

  @override
  void dispose() {
    _pinCtrl.dispose();
    _confirmCtrl.dispose();
    super.dispose();
  }

  void _submit() {
    final pin = _pinCtrl.text;
    if (pin.length < BiometricService.kMinPinLength) {
      setState(() => _error =
          'PIN must be at least ${BiometricService.kMinPinLength} digits');
      return;
    }
    if (widget.confirm && _confirmCtrl.text != pin) {
      setState(() => _error = 'PINs do not match');
      return;
    }
    Navigator.pop(context, pin);
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: Text(widget.title),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          TextField(
            controller: _pinCtrl,
            keyboardType: TextInputType.number,
            obscureText: true,
            maxLength: BiometricService.kMaxPinLength,
            decoration: const InputDecoration(labelText: 'PIN'),
          ),
          if (widget.confirm)
            TextField(
              controller: _confirmCtrl,
              keyboardType: TextInputType.number,
              obscureText: true,
              maxLength: BiometricService.kMaxPinLength,
              decoration: const InputDecoration(labelText: 'Confirm PIN'),
            ),
          if (_error != null)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Text(
                _error!,
                style: const TextStyle(color: Colors.redAccent),
              ),
            ),
        ],
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context),
          child: const Text('Cancel'),
        ),
        TextButton(onPressed: _submit, child: const Text('OK')),
      ],
    );
  }
}
