// Added: Dev/prod flavor configuration for TMAIL-139
// PURPOSE: Resolve the API base URL (and other env-specific settings) from
//          compile-time --dart-define flags. Defaults to `dev` so debug
//          builds keep working without flags.
// EXTERNAL: Read at startup by ApiClient and anywhere else that needs to
//          know which environment the app is running against.
//
// Usage:
//   flutter run --dart-define=ENV=dev     # default; talks to local backend
//   flutter run --dart-define=ENV=prod    # talks to mail.techatscale.io
//   flutter run --dart-define=API_BASE_URL=https://staging.example.com/api
//     # explicit override wins over ENV; useful for staging / branch previews

enum Environment { dev, prod }

class AppEnvironment {
  final Environment environment;
  final String apiBaseUrl;

  const AppEnvironment._({
    required this.environment,
    required this.apiBaseUrl,
  });

  // NOTE: 10.0.2.2 is the Android emulator's loopback to the host machine.
  //       Physical devices need an override via --dart-define=API_BASE_URL.
  static const String devBaseUrl = 'http://10.0.2.2:3000/api';
  static const String prodBaseUrl = 'https://mail.techatscale.io/api';

  // PURPOSE: Resolve the active environment from build-time defines.
  //          Resolution order:
  //            1. --dart-define=API_BASE_URL=...  (explicit URL wins)
  //            2. --dart-define=ENV=prod|dev      (selects preset)
  //            3. Environment.dev fallback        (debug-friendly default)
  static AppEnvironment resolve({
    String envName = const String.fromEnvironment('ENV', defaultValue: 'dev'),
    String apiBaseUrlOverride =
        const String.fromEnvironment('API_BASE_URL', defaultValue: ''),
  }) {
    final env = _parseEnv(envName);
    final baseUrl = apiBaseUrlOverride.isNotEmpty
        ? apiBaseUrlOverride
        : (env == Environment.prod ? prodBaseUrl : devBaseUrl);
    return AppEnvironment._(environment: env, apiBaseUrl: baseUrl);
  }

  static Environment _parseEnv(String name) {
    switch (name.trim().toLowerCase()) {
      case 'prod':
      case 'production':
        return Environment.prod;
      case 'dev':
      case 'development':
      case '':
        return Environment.dev;
      default:
        // NOTE: Unknown ENV values fall back to dev rather than crashing —
        //       protects accidentally typo'd CI configs.
        return Environment.dev;
    }
  }

  bool get isProd => environment == Environment.prod;
  bool get isDev => environment == Environment.dev;

  // Added: Module-level singleton resolved once at startup. Tests can build
  //        their own instances via resolve(envName: ..., apiBaseUrlOverride: ...).
  static final AppEnvironment current = resolve();
}
