// Added: TMAIL-145 — TextField with contact-autocomplete overlay for compose.
// PURPOSE: Drop-in replacement for the plain To/Cc TextFields. Suggestions are
//          keyed off the LAST comma-separated token, so the user can edit
//          recipient N+1 without losing 1..N. Selecting a suggestion appends
//          "Display Name <email>, " — the trailing ", " gives an obvious next
//          insertion point and keeps the regex-free split-on-comma already
//          used by ComposeScreen._send() working.
// EXTERNAL: ContactSuggestionService → GET /api/contacts?q=<token>.

import 'dart:async';

import 'package:flutter/material.dart';

import '../models/contact_suggestion.dart';
import '../services/contact_suggestion_service.dart';

class EmailRecipientField extends StatefulWidget {
  final TextEditingController controller;
  final FocusNode? focusNode;
  final Key? fieldKey;
  final String label;
  final Widget? prefixIcon;
  final Widget? suffixIcon;
  final ContactSuggestionService suggestionService;
  // NOTE: short debounce — typical typing is ~5 cps; 200 ms collapses a
  //       multi-keystroke burst into one HTTP call without feeling laggy.
  final Duration debounce;

  const EmailRecipientField({
    super.key,
    required this.controller,
    required this.suggestionService,
    required this.label,
    this.focusNode,
    this.fieldKey,
    this.prefixIcon,
    this.suffixIcon,
    this.debounce = const Duration(milliseconds: 200),
  });

  @override
  State<EmailRecipientField> createState() => _EmailRecipientFieldState();
}

class _EmailRecipientFieldState extends State<EmailRecipientField> {
  late final FocusNode _focusNode = widget.focusNode ?? FocusNode();
  // Tracks the most-recently-typed token so a stale fetch (older keystroke)
  // can short-circuit instead of clobbering the overlay with old results.
  String _latestToken = '';

  @override
  void dispose() {
    if (widget.focusNode == null) _focusNode.dispose();
    super.dispose();
  }

  // The "current token" is everything after the last comma — that's what the
  // user is actively typing. Trimmed because users often type ", foo".
  String _currentToken(String text) {
    final commaIdx = text.lastIndexOf(',');
    return (commaIdx == -1 ? text : text.substring(commaIdx + 1)).trim();
  }

  // Replaces the last token with the picked suggestion and appends ", " so
  // the caret rests at the start of a fresh slot. Keeps any leading tokens
  // (and the comma between them) verbatim — the user's prior typing is
  // sacred.
  String applySelection(String currentText, ContactSuggestion suggestion) {
    final commaIdx = currentText.lastIndexOf(',');
    final prefix = commaIdx == -1 ? '' : currentText.substring(0, commaIdx + 1);
    final separator = prefix.isEmpty ? '' : ' ';
    return '$prefix$separator${suggestion.formatted()}, ';
  }

  Future<Iterable<ContactSuggestion>> _optionsBuilder(
    TextEditingValue value,
  ) async {
    final token = _currentToken(value.text);
    _latestToken = token;
    if (token.length < 2) return const [];
    if (widget.debounce > Duration.zero) {
      await Future<void>.delayed(widget.debounce);
      // If the user kept typing during the debounce, abort: a newer call
      // will fetch the right thing.
      if (_latestToken != token) return const [];
    }
    return await widget.suggestionService.suggest(token);
  }

  void _onSelected(ContactSuggestion suggestion) {
    final newText = applySelection(widget.controller.text, suggestion);
    widget.controller.value = TextEditingValue(
      text: newText,
      selection: TextSelection.collapsed(offset: newText.length),
    );
  }

  @override
  Widget build(BuildContext context) {
    return RawAutocomplete<ContactSuggestion>(
      textEditingController: widget.controller,
      focusNode: _focusNode,
      optionsBuilder: _optionsBuilder,
      displayStringForOption: (s) => s.formatted(),
      onSelected: _onSelected,
      fieldViewBuilder: (context, controller, focusNode, onFieldSubmitted) {
        return TextField(
          key: widget.fieldKey,
          controller: controller,
          focusNode: focusNode,
          decoration: InputDecoration(
            labelText: widget.label,
            border: const OutlineInputBorder(),
            prefixIcon: widget.prefixIcon,
            suffixIcon: widget.suffixIcon,
          ),
          keyboardType: TextInputType.emailAddress,
          onSubmitted: (_) => onFieldSubmitted(),
        );
      },
      optionsViewBuilder: (context, onSelected, options) {
        return Align(
          alignment: Alignment.topLeft,
          child: Material(
            elevation: 4,
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxHeight: 240, maxWidth: 360),
              child: ListView.builder(
                key: const Key('contact_suggestions_list'),
                padding: EdgeInsets.zero,
                shrinkWrap: true,
                itemCount: options.length,
                itemBuilder: (context, index) {
                  final option = options.elementAt(index);
                  return ListTile(
                    key: Key('contact_suggestion_${option.email}'),
                    dense: true,
                    title: Text(option.displayName ?? option.email),
                    subtitle: option.displayName == null
                        ? null
                        : Text(option.email),
                    onTap: () => onSelected(option),
                  );
                },
              ),
            ),
          ),
        );
      },
    );
  }
}
