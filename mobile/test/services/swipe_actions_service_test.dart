// Added: Unit tests for SwipeActionsService (TMAIL-148)
// PURPOSE: Verify the load/save round-trip through flutter_secure_storage so
//          the user's swipe-action choice actually survives an app restart.
// EXTERNAL: Mocks the flutter_secure_storage method channel with an in-memory
//          map (same pattern as biometric_service_test.dart).

import 'dart:convert';

import 'package:flutter/services.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/services/swipe_actions_service.dart';
import 'package:tasmail_mobile/services/swipe_preferences.dart';

Map<String, String> _installSecureStorageMock() {
  final store = <String, String>{};
  const channel = MethodChannel('plugins.it_nomads.com/flutter_secure_storage');
  TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
      .setMockMethodCallHandler(channel, (MethodCall call) async {
    final args = (call.arguments as Map?) ?? const {};
    final key = args['key'] as String?;
    switch (call.method) {
      case 'write':
        store[key!] = args['value'] as String;
        return null;
      case 'read':
        return store[key];
      case 'delete':
        store.remove(key);
        return null;
      case 'deleteAll':
        store.clear();
        return null;
      default:
        return null;
    }
  });
  return store;
}

void _uninstallSecureStorageMock() {
  const channel = MethodChannel('plugins.it_nomads.com/flutter_secure_storage');
  TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
      .setMockMethodCallHandler(channel, null);
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  late Map<String, String> storage;
  late SwipeActionsService service;

  setUp(() {
    storage = _installSecureStorageMock();
    service = SwipeActionsService(storage: const FlutterSecureStorage());
  });

  tearDown(_uninstallSecureStorageMock);

  group('SwipeActionsService load()', () {
    test('returns defaults when storage is empty', () async {
      final prefs = await service.load();
      expect(prefs.rightAction, SwipeAction.archive);
      expect(prefs.leftAction, SwipeAction.delete);
      expect(service.isLoaded, isTrue);
    });

    test('hydrates from previously saved JSON', () async {
      storage[SwipeActionsService.kStorageKey] = jsonEncode({
        'right_action': 'mark_unread',
        'left_action': 'toggle_flag',
      });

      final prefs = await service.load();
      expect(prefs.rightAction, SwipeAction.markUnread);
      expect(prefs.leftAction, SwipeAction.toggleFlag);
    });

    test('falls back to defaults on corrupt JSON', () async {
      storage[SwipeActionsService.kStorageKey] = '{not-json';
      final prefs = await service.load();
      expect(prefs.rightAction, SwipeAction.archive);
      expect(prefs.leftAction, SwipeAction.delete);
      expect(service.isLoaded, isTrue);
    });
  });

  group('SwipeActionsService save() and helpers', () {
    test('save() persists the new prefs and notifies listeners', () async {
      var notifyCount = 0;
      service.addListener(() => notifyCount++);

      await service.save(const SwipePreferences(
        rightAction: SwipeAction.toggleFlag,
        leftAction: SwipeAction.markUnread,
      ));

      expect(service.preferences.rightAction, SwipeAction.toggleFlag);
      expect(service.preferences.leftAction, SwipeAction.markUnread);
      expect(notifyCount, 1);

      // Persisted JSON round-trips back through fromJson()
      final raw = storage[SwipeActionsService.kStorageKey];
      expect(raw, isNotNull);
      final decoded = SwipePreferences.fromJson(
        jsonDecode(raw!) as Map<String, dynamic>,
      );
      expect(decoded, service.preferences);
    });

    test('setRight() updates only the right action', () async {
      await service.load();
      await service.setRight(SwipeAction.markUnread);
      expect(service.preferences.rightAction, SwipeAction.markUnread);
      expect(service.preferences.leftAction, SwipeAction.delete);

      // Storage matches in-memory state
      final raw = storage[SwipeActionsService.kStorageKey];
      expect(raw, contains('"right_action":"mark_unread"'));
      expect(raw, contains('"left_action":"delete"'));
    });

    test('setLeft() updates only the left action', () async {
      await service.load();
      await service.setLeft(SwipeAction.none);
      expect(service.preferences.leftAction, SwipeAction.none);
      expect(service.preferences.rightAction, SwipeAction.archive);
    });

    test('reset() restores defaults and overwrites disk', () async {
      await service.save(const SwipePreferences(
        rightAction: SwipeAction.none,
        leftAction: SwipeAction.none,
      ));
      expect(service.preferences.rightAction, SwipeAction.none);

      await service.reset();
      expect(service.preferences.rightAction, SwipeAction.archive);
      expect(service.preferences.leftAction, SwipeAction.delete);

      final raw = storage[SwipeActionsService.kStorageKey];
      expect(raw, contains('"right_action":"archive"'));
      expect(raw, contains('"left_action":"delete"'));
    });
  });

  group('SwipeActionsService restart simulation', () {
    test('a new service instance reads the previously saved value', () async {
      await service.save(const SwipePreferences(
        rightAction: SwipeAction.markUnread,
        leftAction: SwipeAction.toggleFlag,
      ));

      // Simulate app restart: spin up a fresh service that points at the same
      // mocked storage backend.
      final reborn = SwipeActionsService(storage: const FlutterSecureStorage());
      final prefs = await reborn.load();
      expect(prefs.rightAction, SwipeAction.markUnread);
      expect(prefs.leftAction, SwipeAction.toggleFlag);
    });
  });
}
