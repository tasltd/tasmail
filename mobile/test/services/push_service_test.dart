// Added: Unit tests for PushService (TMAIL-150)
//
// Strategy: inject a recording fake of PushHttpClient so we exercise the full
// request shape (method + path + body) AND the response handling (success →
// true, exception → false, malformed JSON → safe default) without standing up
// Dio or flutter_secure_storage.
//
// Coverage:
//   * registerToken — body uses backend's `device_token` field (not `token`),
//     platform + device_name + app_version are forwarded, returns true on 2xx,
//     false on thrown exception.
//   * listDevices — parses a List response into a List<Map>, returns [] on
//     error AND on a non-list payload.
//   * unregisterDevice — DELETE on the correct path, returns true / false.
//   * sendTestPush — POST /push/test with no body.
//   * setQuietHours — PUT correct path + body, allows nulls (clear-window),
//     returns false on exception.
//   * syncBadgeCount — PUT correct path + body, returns false on exception.

import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/services/push_service.dart';

class _RecordedCall {
  final String method;
  final String path;
  final dynamic data;
  final Map<String, dynamic>? queryParams;
  _RecordedCall(this.method, this.path, {this.data, this.queryParams});
}

class FakePushHttpClient implements PushHttpClient {
  final List<_RecordedCall> calls = [];

  // PURPOSE: Per-method canned response or thrown exception. Tests set these
  //          before invoking the wrapper.
  Response<dynamic>? nextPostResponse;
  Response<dynamic>? nextGetResponse;
  Response<dynamic>? nextPutResponse;
  Response<dynamic>? nextDeleteResponse;
  Object? throwOnPost;
  Object? throwOnGet;
  Object? throwOnPut;
  Object? throwOnDelete;

  Response<dynamic> _okEmpty() => Response(
        requestOptions: RequestOptions(path: ''),
        statusCode: 200,
        data: null,
      );

  @override
  Future<Response<dynamic>> post(String path, {dynamic data}) async {
    calls.add(_RecordedCall('POST', path, data: data));
    if (throwOnPost != null) throw throwOnPost!;
    return nextPostResponse ?? _okEmpty();
  }

  @override
  Future<Response<dynamic>> get(String path,
      {Map<String, dynamic>? queryParams}) async {
    calls.add(_RecordedCall('GET', path, queryParams: queryParams));
    if (throwOnGet != null) throw throwOnGet!;
    return nextGetResponse ?? _okEmpty();
  }

  @override
  Future<Response<dynamic>> put(String path, {dynamic data}) async {
    calls.add(_RecordedCall('PUT', path, data: data));
    if (throwOnPut != null) throw throwOnPut!;
    return nextPutResponse ?? _okEmpty();
  }

  @override
  Future<Response<dynamic>> delete(String path) async {
    calls.add(_RecordedCall('DELETE', path));
    if (throwOnDelete != null) throw throwOnDelete!;
    return nextDeleteResponse ?? _okEmpty();
  }
}

void main() {
  late FakePushHttpClient http;
  late PushService service;

  setUp(() {
    http = FakePushHttpClient();
    service = PushService(http: http);
  });

  group('registerToken', () {
    test('POSTs to /push/register with backend-shaped body', () async {
      final ok = await service.registerToken(
        token: 'fcm-token-abc',
        platform: 'fcm',
        deviceName: 'Pixel 9',
        appVersion: '1.2.3',
      );

      expect(ok, isTrue);
      expect(http.calls, hasLength(1));
      final c = http.calls.single;
      expect(c.method, 'POST');
      expect(c.path, '/push/register');
      // Critical assertion — the previous bug used `'token'` here and silently
      // failed against the Rust deserializer which requires `device_token`.
      expect(c.data, isA<Map>());
      expect((c.data as Map)['device_token'], 'fcm-token-abc');
      expect((c.data as Map)['platform'], 'fcm');
      expect((c.data as Map)['device_name'], 'Pixel 9');
      expect((c.data as Map)['app_version'], '1.2.3');
    });

    test('omits optional fields by sending nulls (backend treats as Option::None)',
        () async {
      await service.registerToken(token: 't', platform: 'apns');
      final body = http.calls.single.data as Map;
      expect(body['device_token'], 't');
      expect(body['platform'], 'apns');
      expect(body['device_name'], isNull);
      expect(body['app_version'], isNull);
    });

    test('returns false when the HTTP client throws', () async {
      http.throwOnPost = DioException(
        requestOptions: RequestOptions(path: ''),
        type: DioExceptionType.connectionError,
      );
      final ok = await service.registerToken(token: 't', platform: 'fcm');
      expect(ok, isFalse);
    });
  });

  group('listDevices', () {
    test('parses a List<Map> response', () async {
      http.nextGetResponse = Response(
        requestOptions: RequestOptions(path: ''),
        statusCode: 200,
        data: [
          {'id': 'dev1', 'platform': 'fcm'},
          {'id': 'dev2', 'platform': 'apns'},
        ],
      );
      final devices = await service.listDevices();
      expect(devices, hasLength(2));
      expect(devices[0]['id'], 'dev1');
      expect(devices[1]['platform'], 'apns');
      expect(http.calls.single.method, 'GET');
      expect(http.calls.single.path, '/push/devices');
    });

    test('returns [] when the payload is not a list', () async {
      http.nextGetResponse = Response(
        requestOptions: RequestOptions(path: ''),
        statusCode: 200,
        data: {'unexpected': 'shape'},
      );
      expect(await service.listDevices(), isEmpty);
    });

    test('returns [] when the HTTP client throws', () async {
      http.throwOnGet = StateError('boom');
      expect(await service.listDevices(), isEmpty);
    });
  });

  group('unregisterDevice', () {
    test('DELETEs the correct path and returns true', () async {
      final ok = await service.unregisterDevice('dev-uuid-123');
      expect(ok, isTrue);
      expect(http.calls.single.method, 'DELETE');
      expect(http.calls.single.path, '/push/devices/dev-uuid-123');
    });

    test('returns false on exception', () async {
      http.throwOnDelete = StateError('404');
      expect(await service.unregisterDevice('x'), isFalse);
    });
  });

  group('sendTestPush', () {
    test('POSTs /push/test with no body and returns true', () async {
      final ok = await service.sendTestPush();
      expect(ok, isTrue);
      expect(http.calls.single.method, 'POST');
      expect(http.calls.single.path, '/push/test');
      expect(http.calls.single.data, isNull);
    });

    test('returns false on exception', () async {
      http.throwOnPost = StateError('500');
      expect(await service.sendTestPush(), isFalse);
    });
  });

  group('setQuietHours', () {
    test('PUTs to the correct path with the full body', () async {
      final ok = await service.setQuietHours(
        deviceId: 'd1',
        start: '22:00:00',
        end: '06:30:00',
        timezone: 'Africa/Accra',
      );
      expect(ok, isTrue);
      expect(http.calls.single.method, 'PUT');
      expect(http.calls.single.path, '/push/devices/d1/quiet-hours');
      final body = http.calls.single.data as Map;
      expect(body['quiet_hours_start'], '22:00:00');
      expect(body['quiet_hours_end'], '06:30:00');
      expect(body['quiet_hours_timezone'], 'Africa/Accra');
    });

    test('clears the window when all three are null', () async {
      await service.setQuietHours(deviceId: 'd1');
      final body = http.calls.single.data as Map;
      expect(body['quiet_hours_start'], isNull);
      expect(body['quiet_hours_end'], isNull);
      expect(body['quiet_hours_timezone'], isNull);
    });

    test('returns false on exception', () async {
      http.throwOnPut = StateError('boom');
      expect(await service.setQuietHours(deviceId: 'd1'), isFalse);
    });
  });

  group('syncBadgeCount', () {
    test('PUTs to the correct path with badge_count', () async {
      final ok = await service.syncBadgeCount(deviceId: 'd1', badgeCount: 7);
      expect(ok, isTrue);
      expect(http.calls.single.method, 'PUT');
      expect(http.calls.single.path, '/push/devices/d1/badge');
      expect((http.calls.single.data as Map)['badge_count'], 7);
    });

    test('returns false on exception', () async {
      http.throwOnPut = StateError('boom');
      expect(
        await service.syncBadgeCount(deviceId: 'd1', badgeCount: 0),
        isFalse,
      );
    });
  });
}
