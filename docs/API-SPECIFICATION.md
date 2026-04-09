# API Specification
# TASMail — Self-Hosted Email Service

**Version:** 1.0
**Base URL:** `https://mail.example.com/api`
**Content-Type:** `application/json` (unless specified otherwise)

---

## 1. Authentication

### 1.1 Login

```http
POST /api/auth/login
Content-Type: application/json

{
  "username": "user@example.com",
  "password": "secure_password"
}
```

**Response 200:**
```json
{
  "access_token": "eyJhbGciOiJSUzI1NiIs...",
  "token_type": "Bearer",
  "expires_in": 900,
  "user": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "username": "user@example.com",
    "display_name": "John Doe",
    "role": "user"
  }
}
```

**Response Headers:**
```
Set-Cookie: refresh_token=<token>; HttpOnly; Secure; SameSite=Strict; Path=/api/auth; Max-Age=604800
```

**Error 401:** `{ "error": "invalid_credentials", "message": "Invalid username or password" }`
**Error 429:** `{ "error": "rate_limited", "message": "Too many login attempts. Try again in 60 seconds." }`

### 1.2 Refresh Token

```http
POST /api/auth/refresh
Cookie: refresh_token=<token>
```

**Response 200:** Same as login response (new tokens issued, refresh token rotated)
**Error 401:** `{ "error": "invalid_token", "message": "Refresh token expired or invalid" }`

### 1.3 Logout

```http
POST /api/auth/logout
Authorization: Bearer <access_token>
```

**Response 204:** No content. Session deleted. Cookie cleared.

---

## 2. Folders

All folder endpoints require `Authorization: Bearer <token>` header.

### 2.1 List Folders

```http
GET /api/folders
```

**Response 200:**
```json
{
  "folders": [
    {
      "name": "INBOX",
      "display_name": "Inbox",
      "delimiter": "/",
      "flags": ["\\HasNoChildren"],
      "unseen": 12,
      "total": 342,
      "special_use": "\\Inbox"
    },
    {
      "name": "Sent",
      "display_name": "Sent",
      "delimiter": "/",
      "flags": ["\\HasNoChildren", "\\Sent"],
      "unseen": 0,
      "total": 156,
      "special_use": "\\Sent"
    },
    {
      "name": "Drafts",
      "display_name": "Drafts",
      "delimiter": "/",
      "flags": ["\\HasNoChildren", "\\Drafts"],
      "unseen": 0,
      "total": 3,
      "special_use": "\\Drafts"
    },
    {
      "name": "Trash",
      "display_name": "Trash",
      "delimiter": "/",
      "flags": ["\\HasNoChildren", "\\Trash"],
      "unseen": 0,
      "total": 45,
      "special_use": "\\Trash"
    },
    {
      "name": "Projects/ClientA",
      "display_name": "ClientA",
      "delimiter": "/",
      "flags": ["\\HasNoChildren"],
      "unseen": 2,
      "total": 87,
      "special_use": null
    }
  ]
}
```

### 2.2 Create Folder

```http
POST /api/folders
Content-Type: application/json

{
  "name": "Projects/NewClient"
}
```

**Response 201:**
```json
{
  "name": "Projects/NewClient",
  "display_name": "NewClient",
  "delimiter": "/",
  "flags": [],
  "unseen": 0,
  "total": 0,
  "special_use": null
}
```

### 2.3 Rename Folder

```http
PATCH /api/folders/:name
Content-Type: application/json

{
  "new_name": "Projects/RenamedClient"
}
```

**Response 200:** Updated folder object

### 2.4 Delete Folder

```http
DELETE /api/folders/:name
```

**Response 204:** No content
**Error 400:** `{ "error": "cannot_delete", "message": "Cannot delete system folders" }`

---

## 3. Messages

### 3.1 List Messages

```http
GET /api/folders/:folder/messages?page=1&limit=50&sort=date&order=desc
```

**Query Parameters:**

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| page | int | 1 | Page number |
| limit | int | 50 | Messages per page (max 200) |
| sort | string | date | Sort field: date, from, subject, size |
| order | string | desc | Sort order: asc, desc |

**Response 200:**
```json
{
  "messages": [
    {
      "uid": 12345,
      "message_id": "<abc123@example.com>",
      "from": { "name": "John Doe", "email": "john@example.com" },
      "to": [{ "name": "Me", "email": "me@example.com" }],
      "cc": [],
      "subject": "Meeting Tomorrow at 2pm",
      "date": "2026-03-07T14:30:00Z",
      "flags": ["\\Seen"],
      "size": 4523,
      "has_attachments": true,
      "preview": "Hi, just wanted to confirm our meeting..."
    }
  ],
  "pagination": {
    "page": 1,
    "limit": 50,
    "total": 342,
    "pages": 7
  }
}
```

### 3.2 Get Message

```http
GET /api/messages/:uid?folder=INBOX
```

**Response 200:**
```json
{
  "uid": 12345,
  "message_id": "<abc123@example.com>",
  "from": { "name": "John Doe", "email": "john@example.com" },
  "to": [{ "name": "Me", "email": "me@example.com" }],
  "cc": [{ "name": "Boss", "email": "boss@example.com" }],
  "bcc": [],
  "reply_to": { "name": "John Doe", "email": "john@example.com" },
  "subject": "Meeting Tomorrow at 2pm",
  "date": "2026-03-07T14:30:00Z",
  "flags": ["\\Seen"],
  "size": 4523,
  "body": {
    "text": "Hi,\n\nJust wanted to confirm our meeting...\n\nBest,\nJohn",
    "html": "<div><p>Hi,</p><p>Just wanted to confirm our meeting...</p><p>Best,<br>John</p></div>"
  },
  "attachments": [
    {
      "id": "2",
      "filename": "agenda.pdf",
      "mime_type": "application/pdf",
      "size": 102400,
      "content_id": null
    },
    {
      "id": "3",
      "filename": "logo.png",
      "mime_type": "image/png",
      "size": 8192,
      "content_id": "logo123"
    }
  ],
  "headers": {
    "in_reply_to": null,
    "references": [],
    "list_unsubscribe": null
  }
}
```

### 3.3 Send Message

```http
POST /api/messages
Content-Type: multipart/form-data

Fields:
  to:          "John Doe <john@example.com>, jane@example.com"
  cc:          "boss@example.com"                               (optional)
  bcc:         "secret@example.com"                             (optional)
  subject:     "Re: Meeting Tomorrow"
  body_html:   "<p>Confirmed! See you at 2pm.</p>"
  body_text:   "Confirmed! See you at 2pm."                    (optional, auto-generated from HTML if omitted)
  in_reply_to: "<abc123@example.com>"                          (optional)
  references:  "<abc123@example.com>"                          (optional)
  attachments: [file1.pdf, file2.png]                          (optional, multipart files)
```

**Response 201:**
```json
{
  "message_id": "<def456@example.com>",
  "status": "sent"
}
```

**Error 400:** `{ "error": "validation", "message": "At least one recipient required" }`
**Error 413:** `{ "error": "too_large", "message": "Message exceeds 25 MB limit" }`

### 3.4 Update Message Flags

```http
PATCH /api/messages/:uid/flags?folder=INBOX
Content-Type: application/json

{
  "add": ["\\Seen", "\\Flagged"],
  "remove": ["\\Deleted"]
}
```

**Response 200:**
```json
{
  "uid": 12345,
  "flags": ["\\Seen", "\\Flagged"]
}
```

### 3.5 Move Message

```http
POST /api/messages/:uid/move?folder=INBOX
Content-Type: application/json

{
  "destination": "Archive"
}
```

**Response 200:** `{ "uid": 12345, "folder": "Archive" }`

### 3.6 Delete Message

```http
DELETE /api/messages/:uid?folder=INBOX
```

**Behavior:**
- If in non-Trash folder: moves to Trash
- If already in Trash: permanently expunges

**Response 204:** No content

### 3.7 Download Attachment

```http
GET /api/messages/:uid/attachments/:attachment_id?folder=INBOX
```

**Response 200:**
```
Content-Type: application/pdf
Content-Disposition: attachment; filename="agenda.pdf"
Content-Length: 102400

<binary data>
```

---

## 4. Search

### 4.1 Search Messages

```http
GET /api/search?q=meeting&folder=INBOX&from=john&after=2026-01-01&has_attachment=true&page=1&limit=50
```

**Query Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| q | string | Free-text search query |
| folder | string | Folder to search (omit for all folders) |
| from | string | Filter by sender |
| to | string | Filter by recipient |
| subject | string | Filter by subject |
| after | date | Messages after this date (YYYY-MM-DD) |
| before | date | Messages before this date |
| has_attachment | bool | Filter messages with attachments |
| page | int | Page number |
| limit | int | Results per page |

**Response 200:** Same format as message list (3.1)

---

## 5. WebSocket Notifications

### 5.1 Connect

```
WS wss://mail.example.com/ws/notifications?token=<jwt_access_token>
```

### 5.2 Server Events

**New Mail:**
```json
{
  "type": "new_mail",
  "folder": "INBOX",
  "uid": 12346,
  "from": { "name": "Alice", "email": "alice@example.com" },
  "subject": "Quick question",
  "date": "2026-03-07T15:00:00Z",
  "preview": "Hey, do you have a minute..."
}
```

**Flags Changed:**
```json
{
  "type": "flags_changed",
  "folder": "INBOX",
  "uid": 12345,
  "flags": ["\\Seen", "\\Flagged"]
}
```

**Message Expunged:**
```json
{
  "type": "expunge",
  "folder": "INBOX",
  "uid": 12345
}
```

**Heartbeat (every 60s):**
```json
{ "type": "ping" }
```

### 5.3 Client Events

**Pong:**
```json
{ "type": "pong" }
```

**Subscribe to Folder:**
```json
{
  "type": "subscribe",
  "folder": "Projects/ClientA"
}
```

---

## 6. Admin API

Admin endpoints require `role: "domain_admin"` or `role: "super_admin"` in JWT.

### 6.1 Domains

```http
GET    /api/admin/domains                          # List all domains
POST   /api/admin/domains                          # Create domain
PATCH  /api/admin/domains/:id                      # Update domain
DELETE /api/admin/domains/:id                       # Delete (deactivate) domain
```

**Create Domain:**
```json
POST { "domain": "newdomain.com" }
Response 201: { "id": "uuid", "domain": "newdomain.com", "active": true, "user_count": 0, "created_at": "..." }
```

### 6.2 Users

```http
GET    /api/admin/users?domain=example.com&page=1   # List users
POST   /api/admin/users                              # Create user
PATCH  /api/admin/users/:id                          # Update user
DELETE /api/admin/users/:id                           # Deactivate user
```

**Create User:**
```json
POST {
  "username": "newuser@example.com",
  "password": "secure_password_123",
  "display_name": "New User",
  "quota": 2147483648,
  "role": "user"
}
Response 201: { "id": "uuid", "username": "newuser@example.com", ... }
```

### 6.3 Aliases

```http
GET    /api/admin/aliases?domain=example.com        # List aliases
POST   /api/admin/aliases                            # Create alias
DELETE /api/admin/aliases/:id                         # Delete alias
```

**Create Alias:**
```json
POST {
  "source": "info@example.com",
  "destination": "admin@example.com"
}
Response 201: { "id": "uuid", "source": "info@example.com", "destination": "admin@example.com" }
```

### 6.4 System Dashboard

```http
GET /api/admin/stats
```

**Response 200:**
```json
{
  "domains": { "total": 3, "active": 3 },
  "users": { "total": 25, "active": 23 },
  "storage": {
    "total_bytes": 10737418240,
    "used_bytes": 4294967296,
    "usage_percent": 40.0
  },
  "mail_today": { "sent": 45, "received": 128 },
  "queue_size": 2,
  "active_sessions": 8
}
```

---

## 7. Error Response Format

All errors follow a consistent format:

```json
{
  "error": "error_code",
  "message": "Human-readable description",
  "details": {}
}
```

**Standard HTTP Status Codes:**

| Code | Meaning | When |
|------|---------|------|
| 200 | OK | Successful GET/PATCH |
| 201 | Created | Successful POST |
| 204 | No Content | Successful DELETE |
| 400 | Bad Request | Validation error |
| 401 | Unauthorized | Missing/invalid/expired JWT |
| 403 | Forbidden | Insufficient role |
| 404 | Not Found | Resource doesn't exist |
| 409 | Conflict | Duplicate resource (domain, alias) |
| 413 | Payload Too Large | Attachment > 25 MB |
| 429 | Too Many Requests | Rate limit exceeded |
| 500 | Internal Server Error | Unexpected backend error |
| 502 | Bad Gateway | IMAP/SMTP connection failure |

---

## 8. Rate Limits

| Endpoint | Limit | Window |
|----------|-------|--------|
| POST /api/auth/login | 10 | Per minute per IP |
| POST /api/messages (send) | 30 | Per minute per user |
| All other endpoints | 100 | Per minute per user |

Rate limit headers included in all responses:
```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 87
X-RateLimit-Reset: 1709827260
```
