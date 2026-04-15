// Added: Unit tests for auth models for TMAIL-141
import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/models/auth.dart';

void main() {
  group('LoginRequest', () {
    test('serializes to JSON', () {
      const req = LoginRequest(email: 'test@example.com', password: 'pass123');
      final json = req.toJson();
      expect(json['email'], 'test@example.com');
      expect(json['password'], 'pass123');
    });
  });

  group('LoginResponse', () {
    test('parses from JSON', () {
      final json = {
        'access_token': 'jwt-access-token',
        'refresh_token': 'jwt-refresh-token',
        'user': {
          'id': 'user-1',
          'email': 'test@example.com',
          'display_name': 'Test User',
          'avatar_url': null,
        },
      };

      final resp = LoginResponse.fromJson(json);
      expect(resp.accessToken, 'jwt-access-token');
      expect(resp.refreshToken, 'jwt-refresh-token');
      expect(resp.user.id, 'user-1');
      expect(resp.user.email, 'test@example.com');
      expect(resp.user.displayName, 'Test User');
      expect(resp.user.avatarUrl, isNull);
    });
  });

  group('UserInfo', () {
    test('round-trips through JSON', () {
      const user = UserInfo(
        id: 'u-123',
        email: 'alice@example.com',
        displayName: 'Alice',
        avatarUrl: 'https://example.com/avatar.png',
      );

      final json = user.toJson();
      final restored = UserInfo.fromJson(json);
      expect(restored.id, 'u-123');
      expect(restored.email, 'alice@example.com');
      expect(restored.displayName, 'Alice');
      expect(restored.avatarUrl, 'https://example.com/avatar.png');
    });

    test('handles null optional fields', () {
      final json = {
        'id': 'u-456',
        'email': 'bob@example.com',
      };

      final user = UserInfo.fromJson(json);
      expect(user.displayName, isNull);
      expect(user.avatarUrl, isNull);
    });
  });
}
