// Added: Settings API client for TMAIL-152
// PURPOSE: Thin Dio wrapper that hides URL/shape details from the settings
//          screens. One class per concern keeps screen widgets free of
//          serialisation logic and makes mocking trivial in widget tests.
// EXTERNAL: All endpoints sit behind the JWT auth middleware in
//           backend/src/router.rs. Shapes mirror the Rust handler structs.

import 'package:dio/dio.dart';
import 'api_client.dart';

/// Server-side signature record returned by GET /api/signatures.
class SignatureRecord {
  final String id;
  final String name;
  final String htmlBody;
  final String textBody;
  final bool isDefault;

  SignatureRecord({
    required this.id,
    required this.name,
    required this.htmlBody,
    required this.textBody,
    required this.isDefault,
  });

  factory SignatureRecord.fromJson(Map<String, dynamic> json) =>
      SignatureRecord(
        id: json['id'] as String,
        name: (json['name'] ?? '') as String,
        htmlBody: (json['html_body'] ?? '') as String,
        textBody: (json['text_body'] ?? '') as String,
        isDefault: (json['is_default'] ?? false) as bool,
      );
}

/// Contact record returned by GET /api/contacts.
class ContactRecord {
  final String id;
  final String email;
  final String? displayName;
  final String? company;
  final String? phone;

  ContactRecord({
    required this.id,
    required this.email,
    this.displayName,
    this.company,
    this.phone,
  });

  factory ContactRecord.fromJson(Map<String, dynamic> json) => ContactRecord(
        id: json['id'] as String,
        email: (json['email'] ?? '') as String,
        displayName: json['display_name'] as String?,
        company: json['company'] as String?,
        phone: json['phone'] as String?,
      );
}

/// TOTP enrolment payload from POST /api/2fa/enroll.
class TwoFactorEnrollment {
  final String secret;
  final String otpauthUrl;
  final List<String> backupCodes;

  TwoFactorEnrollment({
    required this.secret,
    required this.otpauthUrl,
    required this.backupCodes,
  });

  factory TwoFactorEnrollment.fromJson(Map<String, dynamic> json) =>
      TwoFactorEnrollment(
        secret: json['secret'] as String,
        otpauthUrl: json['otpauth_url'] as String,
        backupCodes: List<String>.from(
          (json['backup_codes'] as List?) ?? const <String>[],
        ),
      );
}

/// Status payload from GET /api/2fa/status.
class TwoFactorStatus {
  final bool enabled;
  final int backupCodesRemaining;

  TwoFactorStatus({required this.enabled, required this.backupCodesRemaining});

  factory TwoFactorStatus.fromJson(Map<String, dynamic> json) => TwoFactorStatus(
        enabled: (json['enabled'] ?? false) as bool,
        backupCodesRemaining:
            (json['backup_codes_remaining'] ?? 0) as int,
      );
}

/// Quota payload from GET /api/quota.
class QuotaStatus {
  final int usedBytes;
  final int quotaBytes;
  final int messageCount;
  final double usagePercent;
  final bool isWarning;
  final bool isOverQuota;

  QuotaStatus({
    required this.usedBytes,
    required this.quotaBytes,
    required this.messageCount,
    required this.usagePercent,
    required this.isWarning,
    required this.isOverQuota,
  });

  factory QuotaStatus.fromJson(Map<String, dynamic> json) => QuotaStatus(
        usedBytes: (json['used_bytes'] ?? 0) as int,
        quotaBytes: (json['quota_bytes'] ?? 0) as int,
        messageCount: (json['message_count'] ?? 0) as int,
        usagePercent: ((json['usage_percent'] ?? 0) as num).toDouble(),
        isWarning: (json['is_warning'] ?? false) as bool,
        isOverQuota: (json['is_over_quota'] ?? false) as bool,
      );
}

/// Push device payload from GET /api/push/devices.
class PushDeviceRecord {
  final String id;
  final String platform;
  final String? deviceName;
  final bool active;

  PushDeviceRecord({
    required this.id,
    required this.platform,
    this.deviceName,
    required this.active,
  });

  factory PushDeviceRecord.fromJson(Map<String, dynamic> json) =>
      PushDeviceRecord(
        id: json['id'] as String,
        platform: (json['platform'] ?? '') as String,
        deviceName: json['device_name'] as String?,
        active: (json['active'] ?? true) as bool,
      );
}

/// PURPOSE: Single dependency-injectable surface for the settings sub-screens.
///          Each method returns a small typed record so the widget layer
///          never touches Dio Responses directly.
class SettingsApi {
  final ApiClient _client;

  SettingsApi({ApiClient? client}) : _client = client ?? ApiClient();

  // --- Signatures ---

  Future<List<SignatureRecord>> listSignatures() async {
    final res = await _client.get('/signatures');
    final list = (res.data as List).cast<Map<String, dynamic>>();
    return list.map(SignatureRecord.fromJson).toList();
  }

  Future<SignatureRecord> createSignature({
    required String name,
    required String htmlBody,
    required String textBody,
    bool isDefault = false,
  }) async {
    final res = await _client.post('/signatures', data: {
      'name': name,
      'html_body': htmlBody,
      'text_body': textBody,
      'is_default': isDefault,
    });
    return SignatureRecord.fromJson(res.data as Map<String, dynamic>);
  }

  Future<SignatureRecord> updateSignature({
    required String id,
    String? name,
    String? htmlBody,
    String? textBody,
    bool? isDefault,
  }) async {
    final body = <String, dynamic>{};
    if (name != null) body['name'] = name;
    if (htmlBody != null) body['html_body'] = htmlBody;
    if (textBody != null) body['text_body'] = textBody;
    if (isDefault != null) body['is_default'] = isDefault;
    final res = await _client.put('/signatures/$id', data: body);
    return SignatureRecord.fromJson(res.data as Map<String, dynamic>);
  }

  Future<void> deleteSignature(String id) async {
    await _client.delete('/signatures/$id');
  }

  // --- Contacts ---

  Future<List<ContactRecord>> listContacts({String? query}) async {
    final res = await _client.get(
      '/contacts',
      queryParams: query != null && query.isNotEmpty ? {'q': query} : null,
    );
    final list = (res.data as List).cast<Map<String, dynamic>>();
    return list.map(ContactRecord.fromJson).toList();
  }

  // --- 2FA ---

  Future<TwoFactorStatus> twoFactorStatus() async {
    final res = await _client.get('/2fa/status');
    return TwoFactorStatus.fromJson(res.data as Map<String, dynamic>);
  }

  Future<TwoFactorEnrollment> twoFactorEnroll() async {
    final res = await _client.post('/2fa/enroll');
    return TwoFactorEnrollment.fromJson(res.data as Map<String, dynamic>);
  }

  Future<void> twoFactorVerify(String code) async {
    await _client.post('/2fa/verify', data: {'code': code});
  }

  Future<void> twoFactorDisable() async {
    await _client.delete('/2fa');
  }

  // --- Quota ---

  Future<QuotaStatus> quota() async {
    final res = await _client.get('/quota');
    return QuotaStatus.fromJson(res.data as Map<String, dynamic>);
  }

  Future<QuotaStatus> syncQuota() async {
    final res = await _client.post('/quota/sync');
    return QuotaStatus.fromJson(res.data as Map<String, dynamic>);
  }

  // --- Push devices (for notification preferences screen) ---

  Future<List<PushDeviceRecord>> listPushDevices() async {
    final res = await _client.get('/push/devices');
    final list = (res.data as List).cast<Map<String, dynamic>>();
    return list.map(PushDeviceRecord.fromJson).toList();
  }

  Future<void> sendTestPush() async {
    await _client.post('/push/test');
  }

  Future<void> deletePushDevice(String id) async {
    await _client.delete('/push/devices/$id');
  }
}

/// PURPOSE: Hide the difference between a Dio failure (network/4xx/5xx) and
///          any other surprise. Settings screens just want a short, human
///          message to drop into a SnackBar.
String describeError(Object error) {
  if (error is DioException) {
    final status = error.response?.statusCode;
    final body = error.response?.data;
    if (body is Map<String, dynamic>) {
      final msg = body['error'] ?? body['message'];
      if (msg is String && msg.isNotEmpty) {
        return status != null ? '$msg ($status)' : msg;
      }
    }
    if (status != null) return 'Request failed ($status)';
    return error.message ?? 'Network error';
  }
  return error.toString();
}
