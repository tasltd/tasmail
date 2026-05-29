// Added: AI-parsed search params display for TMAIL-147
// PURPOSE: Show the user what the LLM extracted from their natural language
//          query as a row of chips, so they can confirm the search interpretation.
// EXTERNAL: Consumes ParsedSearchParams from models/nlp_search.dart.

import 'package:flutter/material.dart';
import '../models/nlp_search.dart';

class ParsedParamsBanner extends StatelessWidget {
  final ParsedSearchParams params;

  const ParsedParamsBanner({super.key, required this.params});

  @override
  Widget build(BuildContext context) {
    if (params.isEmpty) {
      return const SizedBox.shrink();
    }

    final theme = Theme.of(context);
    final chips = <Widget>[];

    void addChip(String label, IconData icon) {
      chips.add(
        Chip(
          avatar: Icon(icon, size: 16, color: theme.colorScheme.primary),
          label: Text(label),
          backgroundColor:
              theme.colorScheme.primaryContainer.withValues(alpha: 0.4),
          visualDensity: VisualDensity.compact,
        ),
      );
    }

    if (params.from != null) addChip('from: ${params.from}', Icons.person_outline);
    if (params.to != null) addChip('to: ${params.to}', Icons.send_outlined);
    if (params.subject != null) addChip('subject: ${params.subject}', Icons.subject);
    for (final kw in params.keywords) {
      addChip(kw, Icons.label_outline);
    }
    if (params.dateFrom != null) {
      addChip('after ${params.dateFrom}', Icons.calendar_today_outlined);
    }
    if (params.dateTo != null) {
      addChip('before ${params.dateTo}', Icons.event_outlined);
    }
    if (params.folder != null) addChip(params.folder!, Icons.folder_outlined);
    if (params.hasAttachment == true) {
      addChip('has attachment', Icons.attach_file);
    }

    return Container(
      key: const Key('parsed_params_banner'),
      width: double.infinity,
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      color: theme.colorScheme.surfaceContainerHighest.withValues(alpha: 0.3),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(Icons.auto_awesome,
                  size: 14, color: theme.colorScheme.onSurfaceVariant),
              const SizedBox(width: 6),
              Text(
                'AI parsed your query as:',
                style: theme.textTheme.labelSmall?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
            ],
          ),
          const SizedBox(height: 6),
          Wrap(spacing: 6, runSpacing: 4, children: chips),
        ],
      ),
    );
  }
}
