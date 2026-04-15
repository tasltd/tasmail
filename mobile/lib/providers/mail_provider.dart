// Added: Mail state provider for TMAIL-143
// PURPOSE: Manages inbox data, folder selection, message loading, and pagination
// EXTERNAL: Uses /api/mobile/* endpoints via ApiClient

import 'package:flutter/material.dart';
import '../api/api_client.dart';
import '../models/email.dart';

class MailProvider extends ChangeNotifier {
  final ApiClient _api = ApiClient();

  // Added: Inbox state
  List<MobileMessageSummary> _messages = [];
  bool _isLoadingInbox = false;
  String? _inboxError;
  int _currentPage = 1;
  int _totalCount = 0;
  bool _hasMore = true;

  // Added: Folder state
  List<MobileFolderSummary> _folders = [];
  String _selectedFolder = 'INBOX';
  bool _isLoadingFolders = false;

  // Added: Unread count
  int _totalUnreadCount = 0;

  // Added: Current message detail
  MobileMessageDetail? _currentMessage;
  bool _isLoadingMessage = false;

  // Getters
  List<MobileMessageSummary> get messages => _messages;
  bool get isLoadingInbox => _isLoadingInbox;
  String? get inboxError => _inboxError;
  bool get hasMore => _hasMore;
  List<MobileFolderSummary> get folders => _folders;
  String get selectedFolder => _selectedFolder;
  bool get isLoadingFolders => _isLoadingFolders;
  int get totalUnreadCount => _totalUnreadCount;
  MobileMessageDetail? get currentMessage => _currentMessage;
  bool get isLoadingMessage => _isLoadingMessage;

  // PURPOSE: Load inbox messages for the selected folder
  Future<void> loadInbox({bool refresh = false}) async {
    if (refresh) {
      _currentPage = 1;
      _messages = [];
      _hasMore = true;
    }

    if (_isLoadingInbox || !_hasMore) return;

    _isLoadingInbox = true;
    _inboxError = null;
    notifyListeners();

    try {
      final response = await _api.get('/mobile/inbox', queryParams: {
        'folder': _selectedFolder,
        'page': _currentPage,
        'per_page': 20,
      });

      final inbox = InboxResponse.fromJson(response.data as Map<String, dynamic>);
      _totalCount = inbox.totalCount;

      if (refresh) {
        _messages = inbox.messages;
      } else {
        _messages.addAll(inbox.messages);
      }

      _hasMore = _messages.length < _totalCount;
      _currentPage++;
    } catch (e) {
      _inboxError = 'Failed to load messages';
    } finally {
      _isLoadingInbox = false;
      notifyListeners();
    }
  }

  // PURPOSE: Load next page (infinite scroll)
  Future<void> loadMore() async {
    if (!_hasMore || _isLoadingInbox) return;
    await loadInbox();
  }

  // PURPOSE: Load folder list with unread counts
  Future<void> loadFolders() async {
    _isLoadingFolders = true;
    notifyListeners();

    try {
      final response = await _api.get('/mobile/folders');
      _folders = (response.data as List<dynamic>)
          .map((f) => MobileFolderSummary.fromJson(f as Map<String, dynamic>))
          .toList();
    } catch (_) {
      // NOTE: Keep existing folders on error
    } finally {
      _isLoadingFolders = false;
      notifyListeners();
    }
  }

  // PURPOSE: Switch to a different folder
  Future<void> selectFolder(String folder) async {
    _selectedFolder = folder;
    await loadInbox(refresh: true);
  }

  // PURPOSE: Fetch total unread count across all folders
  Future<void> loadUnreadCount() async {
    try {
      final response = await _api.get('/mobile/unread-count');
      _totalUnreadCount = response.data['total_unseen'] as int? ?? 0;
      notifyListeners();
    } catch (_) {
      // NOTE: Non-critical, keep previous count
    }
  }

  // PURPOSE: Load full message detail
  Future<void> loadMessage(String folder, int uid) async {
    _isLoadingMessage = true;
    _currentMessage = null;
    notifyListeners();

    try {
      final response = await _api.get('/mobile/message/$folder/$uid');
      _currentMessage = MobileMessageDetail.fromJson(
        response.data as Map<String, dynamic>,
      );
    } catch (_) {
      _currentMessage = null;
    } finally {
      _isLoadingMessage = false;
      notifyListeners();
    }
  }

  // PURPOSE: Delete a message (move to Trash)
  Future<bool> deleteMessage(String folder, int uid) async {
    try {
      await _api.delete('/folders/$folder/messages/$uid');
      _messages.removeWhere((m) => m.uid == uid && m.folder == folder);
      notifyListeners();
      return true;
    } catch (_) {
      return false;
    }
  }

  // PURPOSE: Toggle flag on a message
  Future<bool> toggleFlag(String folder, int uid, bool flagged) async {
    try {
      await _api.put('/folders/$folder/messages/$uid/flag', data: {
        'flagged': flagged,
      });
      // NOTE: Update local state optimistically
      final index = _messages.indexWhere((m) => m.uid == uid && m.folder == folder);
      if (index != -1) {
        final old = _messages[index];
        _messages[index] = MobileMessageSummary(
          uid: old.uid,
          folder: old.folder,
          from: old.from,
          subject: old.subject,
          date: old.date,
          isRead: old.isRead,
          isFlagged: flagged,
          hasAttachment: old.hasAttachment,
        );
        notifyListeners();
      }
      return true;
    } catch (_) {
      return false;
    }
  }

  // PURPOSE: Mark message as read
  Future<void> markAsRead(String folder, int uid) async {
    try {
      await _api.put('/folders/$folder/messages/$uid/read');
      final index = _messages.indexWhere((m) => m.uid == uid && m.folder == folder);
      if (index != -1) {
        final old = _messages[index];
        _messages[index] = MobileMessageSummary(
          uid: old.uid,
          folder: old.folder,
          from: old.from,
          subject: old.subject,
          date: old.date,
          isRead: true,
          isFlagged: old.isFlagged,
          hasAttachment: old.hasAttachment,
        );
        notifyListeners();
      }
    } catch (_) {
      // NOTE: Non-critical, will sync on next load
    }
  }
}
