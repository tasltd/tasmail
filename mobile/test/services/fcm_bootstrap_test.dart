// Added: Unit tests for FcmBootstrap (TMAIL-150)
//
// Strategy: drive the bootstrap with fake token providers + a recording
// navigator callback + a fake PushHttpClient under PushService. No Firebase
// dependency, no NavigatorState, no platform channels.
//
// Coverage:
//   * register() — no-op when token provider returns null/empty (steady state
//     today without Firebase config); calls /push/register exactly once when a
//     token IS available; idempotent on repeat calls with the same token;
//     re-registers when token changes; tracks _lastRegisteredToken only on
//     backend success.
//   * subscribeToTokenRefresh — registers each emitted token; cancel function
//     stops further registration; empty tokens are ignored.
//   * handleTap — parses {folder, uid} and navigates to /message; ignores
//     malformed payloads, unknown types, and missing fields without crashing.

import 'dart:async';

import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/services/fcm_bootstrap.dart';
import 'package:tasmail_mobile/services/push_service.dart';

// PURPOSE: Recording fake of PushHttpClient so we assert the exact request
//          the bootstrap → PushService → http stack sends to /push/register.
class _FakeHttp implements PushHttpClient {
  final List<Map<String, dynamic>> registerBodies = [];

  Response<dynamic> _okEmpty() => Response(
        requestOptions: RequestOptions(path: ''),
        statusCode: 200,
        data: null,
      );

  bool shouldFail = false;

  @override
  Future<Response<dynamic>> post(String path, {dynamic data}) async {
    if (path == '/push/register' && data is Map) {
      registerBodies.add(Map<String, dynamic>.from(data));
    }
    if (shouldFail) {
      throw DioException(
        requestOptions: RequestOptions(path: path),
        type: DioExceptionType.connectionError,
      );
    }
    return _okEmpty();
  }

  @override
  Future<Response<dynamic>> get(String path,
          {Map<String, dynamic>? queryParams}) async =>
      _okEmpty();
  @override
  Future<Response<dynamic>> put(String path, {dynamic data}) async =>
      _okEmpty();
  @override
  Future<Response<dynamic>> delete(String path) async => _okEmpty();
}

class _RecordedNav {
  final String routeName;
  final Object? arguments;
  _RecordedNav(this.routeName, this.arguments);
}

void main() {
  late _FakeHttp http;
  late PushService pushService;
  late List<_RecordedNav> navCalls;
  late FcmTapNavigator navigator;

  setUp(() {
    http = _FakeHttp();
    pushService = PushService(http: http);
    navCalls = [];
    navigator = (route, args) => navCalls.add(_RecordedNav(route, args));
  });

  FcmBootstrap newBootstrap({
    FcmTokenProvider? tokenProvider,
    FcmTokenRefreshStream? refreshStream,
    String platform = FcmPlatformId.fcm,
    String? deviceName = 'Pixel 9',
    String? appVersion = '1.0.0',
  }) {
    return FcmBootstrap(
      navigator: navigator,
      platform: platform,
      pushService: pushService,
      tokenProvider: tokenProvider,
      refreshStream: refreshStream,
      deviceName: deviceName,
      appVersion: appVersion,
    );
  }

  group('register()', () {
    test('is a silent no-op when no token provider is configured', () async {
      final boot = newBootstrap();
      final ok = await boot.register();
      expect(ok, isFalse, reason: 'no token => no registration');
      expect(http.registerBodies, isEmpty);
      expect(boot.lastRegisteredToken, isNull);
    });

    test('is a no-op when the token provider returns an empty string',
        () async {
      final boot = newBootstrap(tokenProvider: () async => '');
      expect(await boot.register(), isFalse);
      expect(http.registerBodies, isEmpty);
    });

    test('calls /push/register with the right body when a token is present',
        () async {
      final boot = newBootstrap(tokenProvider: () async => 'fcm-token-abc');
      final ok = await boot.register();

      expect(ok, isTrue);
      expect(http.registerBodies, hasLength(1));
      final body = http.registerBodies.single;
      // Critical — this is the field name the backend expects, not `token`.
      expect(body['device_token'], 'fcm-token-abc');
      expect(body['platform'], 'fcm');
      expect(body['device_name'], 'Pixel 9');
      expect(body['app_version'], '1.0.0');

      expect(boot.lastRegisteredToken, 'fcm-token-abc');
    });

    test('is idempotent for the same token across repeat calls', () async {
      final boot = newBootstrap(tokenProvider: () async => 'same-token');
      expect(await boot.register(), isTrue);
      expect(await boot.register(), isTrue);
      expect(await boot.register(), isTrue);
      expect(http.registerBodies, hasLength(1),
          reason: 'subsequent register() with same token must skip the round trip');
    });

    test('re-registers when the token rotates between calls', () async {
      String token = 'token-1';
      final boot = newBootstrap(tokenProvider: () async => token);
      expect(await boot.register(), isTrue);
      token = 'token-2';
      expect(await boot.register(), isTrue);
      expect(http.registerBodies, hasLength(2));
      expect(http.registerBodies[0]['device_token'], 'token-1');
      expect(http.registerBodies[1]['device_token'], 'token-2');
      expect(boot.lastRegisteredToken, 'token-2');
    });

    test('does not advance lastRegisteredToken on backend failure', () async {
      http.shouldFail = true;
      final boot = newBootstrap(tokenProvider: () async => 'unlucky-token');
      expect(await boot.register(), isFalse);
      expect(boot.lastRegisteredToken, isNull,
          reason: 'failed register must NOT poison the dedupe cache');

      // Once the backend recovers, the very next call should retry — not skip.
      http.shouldFail = false;
      expect(await boot.register(), isTrue);
      expect(http.registerBodies.length, 2,
          reason: 'one failed POST + one successful retry');
    });

    test('honours the platform string passed to the bootstrap', () async {
      final boot = newBootstrap(
        tokenProvider: () async => 'apns-token',
        platform: FcmPlatformId.apns,
      );
      await boot.register();
      expect(http.registerBodies.single['platform'], 'apns');
    });
  });

  group('subscribeToTokenRefresh', () {
    test('registers each emitted token in order', () async {
      final controller = StreamController<String>();
      final boot = newBootstrap(refreshStream: () => controller.stream);
      final cancel = boot.subscribeToTokenRefresh();

      controller.add('rotated-token-a');
      await Future<void>.delayed(Duration.zero);
      controller.add('rotated-token-b');
      await Future<void>.delayed(Duration.zero);

      expect(http.registerBodies, hasLength(2));
      expect(http.registerBodies[0]['device_token'], 'rotated-token-a');
      expect(http.registerBodies[1]['device_token'], 'rotated-token-b');
      expect(boot.lastRegisteredToken, 'rotated-token-b');

      cancel();
      await controller.close();
    });

    test('cancel stops further registrations', () async {
      final controller = StreamController<String>();
      final boot = newBootstrap(refreshStream: () => controller.stream);
      final cancel = boot.subscribeToTokenRefresh();

      controller.add('first');
      await Future<void>.delayed(Duration.zero);
      cancel();
      controller.add('after-cancel-should-not-register');
      await Future<void>.delayed(Duration.zero);

      expect(http.registerBodies, hasLength(1));
      expect(http.registerBodies.single['device_token'], 'first');
      await controller.close();
    });

    test('ignores empty token emissions', () async {
      final controller = StreamController<String>();
      final boot = newBootstrap(refreshStream: () => controller.stream);
      boot.subscribeToTokenRefresh();
      controller.add('');
      await Future<void>.delayed(Duration.zero);
      expect(http.registerBodies, isEmpty);
      await controller.close();
    });
  });

  group('handleTap', () {
    test('navigates to /message for a new_mail payload', () {
      final boot = newBootstrap();
      boot.handleTap({
        'type': 'new_mail',
        'folder': 'INBOX',
        'uid': '12345',
      });
      expect(navCalls, hasLength(1));
      expect(navCalls.single.routeName, '/message');
      expect(navCalls.single.arguments, isA<Map>());
      expect((navCalls.single.arguments as Map)['folder'], 'INBOX');
      expect((navCalls.single.arguments as Map)['uid'], 12345,
          reason: 'uid must be parsed back to int for MessageScreen');
    });

    test('defaults to new_mail when type is omitted (back-compat)', () {
      newBootstrap().handleTap({'folder': 'Sent', 'uid': '7'});
      expect(navCalls.single.routeName, '/message');
      expect((navCalls.single.arguments as Map)['folder'], 'Sent');
      expect((navCalls.single.arguments as Map)['uid'], 7);
    });

    test('ignores payloads with missing folder', () {
      newBootstrap().handleTap({'type': 'new_mail', 'uid': '1'});
      expect(navCalls, isEmpty);
    });

    test('ignores payloads with missing uid', () {
      newBootstrap().handleTap({'type': 'new_mail', 'folder': 'INBOX'});
      expect(navCalls, isEmpty);
    });

    test('ignores payloads with non-numeric uid', () {
      newBootstrap().handleTap(
          {'type': 'new_mail', 'folder': 'INBOX', 'uid': 'not-a-number'});
      expect(navCalls, isEmpty);
    });

    test('test pushes do not navigate (acknowledged silently)', () {
      newBootstrap().handleTap({'type': 'test'});
      expect(navCalls, isEmpty);
    });

    test('unknown push types are ignored without crashing', () {
      newBootstrap().handleTap({'type': 'future_feature_x'});
      expect(navCalls, isEmpty);
    });

    test('empty payload is a no-op', () {
      newBootstrap().handleTap(const {});
      expect(navCalls, isEmpty);
    });
  });
}
