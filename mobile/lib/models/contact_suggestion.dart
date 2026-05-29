// Added: TMAIL-145 — lightweight contact projection for recipient autocomplete.
// PURPOSE: Decode the shape returned by GET /api/contacts (which lists every
//          field of `models::contact::Contact`) but only keep the bits the
//          compose-screen autocomplete needs: a display name and an email.
// EXTERNAL: Backend handler `handlers::contacts::list_contacts` (router.rs:166).

class ContactSuggestion {
  final String email;
  final String? displayName;

  const ContactSuggestion({required this.email, this.displayName});

  factory ContactSuggestion.fromJson(Map<String, dynamic> json) {
    return ContactSuggestion(
      email: (json['email'] as String?)?.trim() ?? '',
      displayName: (json['display_name'] as String?)?.trim().isEmpty == true
          ? null
          : json['display_name'] as String?,
    );
  }

  // PURPOSE: RFC 5322 address form ("Display Name <user@host>") if a display
  //          name exists, otherwise the bare email. The backend's
  //          `validate_recipient_list` accepts both, and lettre parses both.
  String formatted() {
    final name = displayName?.trim() ?? '';
    if (name.isEmpty) return email;
    return '$name <$email>';
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is ContactSuggestion &&
          other.email == email &&
          other.displayName == displayName;

  @override
  int get hashCode => Object.hash(email, displayName);
}
