// Added: Unit tests for AppEnvironment dev/prod flavor resolution (TMAIL-139)
// PURPOSE: Validates the full input → output round-trip of resolve():
//          given a build-time ENV / API_BASE_URL pair, the right Environment
//          enum value AND the right base URL come out.

import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/utils/app_environment.dart';

void main() {
  group('AppEnvironment.resolve', () {
    test('defaults to dev when no flags are supplied', () {
      final env = AppEnvironment.resolve(envName: 'dev', apiBaseUrlOverride: '');

      expect(env.environment, Environment.dev);
      expect(env.apiBaseUrl, AppEnvironment.devBaseUrl);
      expect(env.isDev, isTrue);
      expect(env.isProd, isFalse);
    });

    test('selects prod when ENV=prod', () {
      final env =
          AppEnvironment.resolve(envName: 'prod', apiBaseUrlOverride: '');

      expect(env.environment, Environment.prod);
      expect(env.apiBaseUrl, AppEnvironment.prodBaseUrl);
      expect(env.isProd, isTrue);
      expect(env.isDev, isFalse);
    });

    test('accepts production / development long-form aliases', () {
      final prod = AppEnvironment.resolve(
        envName: 'production',
        apiBaseUrlOverride: '',
      );
      final dev = AppEnvironment.resolve(
        envName: 'development',
        apiBaseUrlOverride: '',
      );

      expect(prod.environment, Environment.prod);
      expect(prod.apiBaseUrl, AppEnvironment.prodBaseUrl);
      expect(dev.environment, Environment.dev);
      expect(dev.apiBaseUrl, AppEnvironment.devBaseUrl);
    });

    test('is case-insensitive and tolerant of whitespace', () {
      final env = AppEnvironment.resolve(
        envName: '  PROD  ',
        apiBaseUrlOverride: '',
      );

      expect(env.environment, Environment.prod);
      expect(env.apiBaseUrl, AppEnvironment.prodBaseUrl);
    });

    test('falls back to dev for an unknown ENV value', () {
      final env = AppEnvironment.resolve(
        envName: 'staging-typo',
        apiBaseUrlOverride: '',
      );

      expect(env.environment, Environment.dev);
      expect(env.apiBaseUrl, AppEnvironment.devBaseUrl);
    });

    test('API_BASE_URL override wins over ENV preset', () {
      const customUrl = 'https://staging.example.com/api';
      final env = AppEnvironment.resolve(
        envName: 'prod',
        apiBaseUrlOverride: customUrl,
      );

      // NOTE: The enum still reports prod (so feature flags keyed on isProd
      //       behave correctly), but the URL is the explicit override.
      expect(env.environment, Environment.prod);
      expect(env.apiBaseUrl, customUrl);
    });

    test('empty API_BASE_URL falls back to the ENV preset URL', () {
      final env = AppEnvironment.resolve(
        envName: 'prod',
        apiBaseUrlOverride: '',
      );

      expect(env.apiBaseUrl, AppEnvironment.prodBaseUrl);
    });
  });

  group('AppEnvironment.current', () {
    test('resolves to a valid environment with a non-empty base URL', () {
      // NOTE: current uses real build-time defines. In `flutter test`
      //       (no --dart-define) it defaults to dev. We only assert
      //       invariants rather than a specific environment, so the
      //       test works under any --dart-define combination.
      final current = AppEnvironment.current;

      expect(Environment.values, contains(current.environment));
      expect(current.apiBaseUrl, isNotEmpty);
      expect(current.apiBaseUrl, startsWith('http'));
    });
  });
}
