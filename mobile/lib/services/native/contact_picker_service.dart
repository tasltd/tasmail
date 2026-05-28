// Added: ContactPickerService for TMAIL-55
// PURPOSE: Open the system contact picker and return an email address that the
//          user selected, so the To/Cc/Bcc fields in ComposeScreen don't require
//          typing. If the picked contact has multiple emails we surface a
//          chooser callback so the caller can present a small dialog.
// EXTERNAL: flutter_contacts (requires READ_CONTACTS permission, requested
//           lazily by the plugin the first time openExternalPick() is called).

import 'package:flutter_contacts/flutter_contacts.dart';

class PickedContact {
  final String? displayName;
  final List<String> emails;

  const PickedContact({this.displayName, required this.emails});

  bool get hasEmail => emails.isNotEmpty;
  String? get primaryEmail => emails.isNotEmpty ? emails.first : null;
}

abstract class ContactPickerService {
  // Launch the OS contact picker. Returns null if the user cancelled or denied
  // permission. Throws nothing — callers should treat null as "no selection".
  Future<PickedContact?> pickContact();
}

class ContactPickerServiceImpl implements ContactPickerService {
  // Injected so tests can stub the static plugin call.
  final Future<Contact?> Function() _openPicker;

  ContactPickerServiceImpl({Future<Contact?> Function()? openPicker})
      : _openPicker = openPicker ?? _defaultOpenPicker;

  static Future<Contact?> _defaultOpenPicker() {
    // NOTE: openExternalPick handles permission prompting on both platforms.
    return FlutterContacts.openExternalPick();
  }

  @override
  Future<PickedContact?> pickContact() async {
    final Contact? contact = await _openPicker();
    if (contact == null) return null;
    final emails = contact.emails
        .map((e) => e.address.trim())
        .where((s) => s.isNotEmpty)
        .toList(growable: false);
    return PickedContact(
      displayName: contact.displayName.isEmpty ? null : contact.displayName,
      emails: emails,
    );
  }
}
