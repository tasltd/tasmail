-- Added (TMAIL-303): auto_create_users flag on SAML configurations.
--
-- When true (default), a successful SAML assertion for an unknown name_id
-- provisions a new mailbox automatically (in the byok.tasmail synthetic
-- domain). When false, the callback returns 403 for unknown subjects and
-- the IdP-side admin must pre-create the mailbox.
--
-- Default true keeps existing demo / staging SAML configs working without
-- explicit migration; production deployments that need stricter control
-- can flip it via the admin endpoint.
ALTER TABLE saml_configurations
    ADD COLUMN auto_create_users BOOLEAN NOT NULL DEFAULT true;
