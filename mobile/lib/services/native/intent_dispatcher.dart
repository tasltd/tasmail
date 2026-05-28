// Added: IntentDispatcher for TMAIL-55
// PURPOSE: Glue that subscribes to ShareIntentService + DeepLinkService and
//          pushes ComposeScreen with the resulting prefill. Kept separate so
//          main.dart stays declarative and so tests can drive prefill events
//          without touching platform channels.
// EXTERNAL: GlobalKey<NavigatorState> from MaterialApp.

import 'dart:async';

import 'package:flutter/material.dart';

import 'deep_link_service.dart';
import 'share_intent_service.dart';

class IntentDispatcher {
  final GlobalKey<NavigatorState> navigatorKey;
  final ShareIntentService shareService;
  final DeepLinkService deepLinkService;

  StreamSubscription<ComposePrefill>? _shareSub;
  StreamSubscription<ComposePrefill>? _linkSub;

  IntentDispatcher({
    required this.navigatorKey,
    required this.shareService,
    required this.deepLinkService,
  });

  // Wire up cold-start handling AND the warm-resume streams.
  Future<void> start() async {
    // NOTE: cold-start checks run sequentially; first non-null wins so the
    //       user doesn't get two compose screens stacked.
    final initialShare = await shareService.initialShare();
    if (initialShare != null && !initialShare.isEmpty) {
      _open(initialShare);
    } else {
      final initialLink = await deepLinkService.initialLink();
      if (initialLink != null && !initialLink.isEmpty) {
        _open(initialLink);
      }
    }
    _shareSub = shareService.incomingShares.listen(_open);
    _linkSub = deepLinkService.incomingLinks.listen(_open);
  }

  Future<void> dispose() async {
    await _shareSub?.cancel();
    await _linkSub?.cancel();
  }

  void _open(ComposePrefill prefill) {
    final nav = navigatorKey.currentState;
    if (nav == null) return;
    nav.pushNamed('/compose', arguments: {'prefill': prefill});
  }
}
