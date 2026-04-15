// Added: Email data models matching backend mobile API responses for TMAIL-143/144
// PURPOSE: Dart classes for MobileMessageSummary, MobileMessageDetail, MobileFolderSummary
// EXTERNAL: Maps to backend/src/models/mobile.rs types

class MobileMessageSummary {
  final int uid;
  final String folder;
  final String? from;
  final String? subject;
  final String? date;
  final bool isRead;
  final bool isFlagged;
  final bool hasAttachment;

  const MobileMessageSummary({
    required this.uid,
    required this.folder,
    this.from,
    this.subject,
    this.date,
    required this.isRead,
    required this.isFlagged,
    required this.hasAttachment,
  });

  factory MobileMessageSummary.fromJson(Map<String, dynamic> json) {
    return MobileMessageSummary(
      uid: json['uid'] as int,
      folder: json['folder'] as String,
      from: json['from'] as String?,
      subject: json['subject'] as String?,
      date: json['date'] as String?,
      isRead: json['is_read'] as bool? ?? false,
      isFlagged: json['is_flagged'] as bool? ?? false,
      hasAttachment: json['has_attachment'] as bool? ?? false,
    );
  }
}

class MobileMessageDetail {
  final int uid;
  final String folder;
  final String? from;
  final List<String> to;
  final List<String> cc;
  final String? subject;
  final String? date;
  final String? bodyHtml;
  final String? bodyText;
  final bool isRead;
  final bool isFlagged;
  final bool hasAttachment;
  final List<AttachmentInfo> attachments;

  const MobileMessageDetail({
    required this.uid,
    required this.folder,
    this.from,
    required this.to,
    required this.cc,
    this.subject,
    this.date,
    this.bodyHtml,
    this.bodyText,
    required this.isRead,
    required this.isFlagged,
    required this.hasAttachment,
    required this.attachments,
  });

  factory MobileMessageDetail.fromJson(Map<String, dynamic> json) {
    return MobileMessageDetail(
      uid: json['uid'] as int,
      folder: json['folder'] as String,
      from: json['from'] as String?,
      to: List<String>.from(json['to'] ?? []),
      cc: List<String>.from(json['cc'] ?? []),
      subject: json['subject'] as String?,
      date: json['date'] as String?,
      bodyHtml: json['body_html'] as String?,
      bodyText: json['body_text'] as String?,
      isRead: json['is_read'] as bool? ?? false,
      isFlagged: json['is_flagged'] as bool? ?? false,
      hasAttachment: json['has_attachment'] as bool? ?? false,
      attachments: (json['attachments'] as List<dynamic>?)
              ?.map((a) => AttachmentInfo.fromJson(a as Map<String, dynamic>))
              .toList() ??
          [],
    );
  }
}

class AttachmentInfo {
  final String id;
  final String filename;
  final String contentType;
  final int sizeBytes;

  const AttachmentInfo({
    required this.id,
    required this.filename,
    required this.contentType,
    required this.sizeBytes,
  });

  factory AttachmentInfo.fromJson(Map<String, dynamic> json) {
    return AttachmentInfo(
      id: json['id'] as String,
      filename: json['filename'] as String,
      contentType: json['content_type'] as String? ?? 'application/octet-stream',
      sizeBytes: json['size_bytes'] as int? ?? 0,
    );
  }
}

class MobileFolderSummary {
  final String name;
  final int unreadCount;
  final int totalCount;

  const MobileFolderSummary({
    required this.name,
    required this.unreadCount,
    required this.totalCount,
  });

  factory MobileFolderSummary.fromJson(Map<String, dynamic> json) {
    return MobileFolderSummary(
      name: json['name'] as String,
      unreadCount: json['unread_count'] as int? ?? 0,
      totalCount: json['total_count'] as int? ?? 0,
    );
  }
}

// Added: Inbox response wrapper with pagination info
class InboxResponse {
  final List<MobileMessageSummary> messages;
  final int totalCount;
  final int page;
  final int perPage;

  const InboxResponse({
    required this.messages,
    required this.totalCount,
    required this.page,
    required this.perPage,
  });

  factory InboxResponse.fromJson(Map<String, dynamic> json) {
    return InboxResponse(
      messages: (json['messages'] as List<dynamic>)
          .map((m) => MobileMessageSummary.fromJson(m as Map<String, dynamic>))
          .toList(),
      totalCount: json['total_count'] as int? ?? 0,
      page: json['page'] as int? ?? 1,
      perPage: json['per_page'] as int? ?? 20,
    );
  }
}
