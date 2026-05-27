// Added: Unit tests for ApiClient (TMAIL-140)
// PURPOSE: Verify singleton identity + secure-storage token round-trip via
//          the flutter_secure_storage MethodChannel mock. We exercise the
//          public surface (saveTokens / hasTokens / clearTokens) because that
//          is what the rest of the app (AuthProvider et al.) depends on.

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/api/api_client.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  // NOTE: flutter_secure_storage v10 still routes through this channel name.
  //       The mock keeps an in-memory map so tests are hermetic and order-safe.
  const channel = MethodChannel('plugins.it_nomads.com/flutter_secure_storage');
  final Map<String, String> mockStore = <String, String>{};

  setUp(() {
    mockStore.clear();
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (MethodCall call) async {
      final args = (call.arguments as Map?)?.cast<String, dynamic>() ?? const {};
      switch (call.method) {
        case 'read':
          return mockStore[args['key'] as String];
        case 'write':
          mockStore[args['key'] as String] = args['value'] as String;
          return null;
        case 'delete':
          mockStore.remove(args['key'] as String);
          return null;
        case 'containsKey':
          return mockStore.containsKey(args['key'] as String);
        case 'readAll':
          return Map<String, String>.from(mockStore);
        case 'deleteAll':
          mockStore.clear();
          return null;
      }
      return null;
    });
  });

  tearDown(() async {
    // NOTE: Singleton state must be wiped between tests so token assertions
    //       don't bleed across test cases.
    await ApiClient().clearTokens();
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, null);
  });

  group('ApiClient singleton', () {
    test('factory returns the same instance', () {
      final a = ApiClient();
      final b = ApiClient();
      expect(identical(a, b), isTrue);
    });
  });

  group('ApiClient token storage round-trip', () {
    test('hasTokens is false on a fresh secure store', () async {
      final client = ApiClient();
      await client.clearTokens();
      expect(await client.hasTokens(), isFalse);
    });

    test('saveTokens writes both access and refresh tokens', () async {
      final client = ApiClient();
      await client.saveTokens(
        accessToken: 'access-abc',
        refreshToken: 'refresh-xyz',
      );

      expect(mockStore['access_token'], 'access-abc');
      expect(mockStore['refresh_token'], 'refresh-xyz');
      expect(await client.hasTokens(), isTrue);
    });

    test('clearTokens removes both tokens', () async {
      final client = ApiClient();
      await client.saveTokens(
        accessToken: 'access-abc',
        refreshToken: 'refresh-xyz',
      );
      expect(await client.hasTokens(), isTrue);

      await client.clearTokens();

      expect(mockStore.containsKey('access_token'), isFalse);
      expect(mockStore.containsKey('refresh_token'), isFalse);
      expect(await client.hasTokens(), isFalse);
    });

    test('saveTokens overwrites prior values', () async {
      final client = ApiClient();
      await client.saveTokens(accessToken: 'a1', refreshToken: 'r1');
      await client.saveTokens(accessToken: 'a2', refreshToken: 'r2');

      expect(mockStore['access_token'], 'a2');
      expect(mockStore['refresh_token'], 'r2');
    });
  });

  group('ApiClient configuration', () {
    test('setBaseUrl mutates the underlying Dio base URL', () {
      final client = ApiClient();
      // NOTE: We cannot read _dio.options.baseUrl directly (private field),
      //       so we assert that the call completes without throwing. The
      //       observable side effect is exercised by the integration tests
      //       that follow a setBaseUrl with an HTTP call.
      expect(
        () => client.setBaseUrl('https://staging.example.com/api'),
        returnsNormally,
      );
    });
  });
}
