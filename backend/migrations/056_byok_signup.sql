-- Added: BYOK (Bring Your Own Key) signup pivot.
-- TASMail's identity is "webmail UI for any IMAP/SMTP server" — users sign up with
-- a TASMail account (just an email + password), then attach their own mail-server
-- credentials (Gmail/Outlook/Yahoo/Zoho/FastMail/Dovecot/Exchange/etc.) via the
-- imap_configurations + smtp_configurations tables.
--
-- The original schema required mailboxes.domain_id (TASMail-as-mail-server design).
-- For BYOK signup we auto-attach new accounts to a synthetic "byok.tasmail" domain
-- so the FK still resolves but the domain has no real mail-server meaning.

INSERT INTO domains (id, name, active, created_at, updated_at)
SELECT gen_random_uuid(), 'byok.tasmail', true, NOW(), NOW()
WHERE NOT EXISTS (SELECT 1 FROM domains WHERE name = 'byok.tasmail');
