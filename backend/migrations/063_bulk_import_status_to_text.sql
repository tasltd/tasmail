-- TMAIL-202: convert bulk_user_imports.status from the bulk_import_status
-- ENUM to TEXT + CHECK so it round-trips through models/bulk_import.rs
-- (which decodes status as `String`).
--
-- Same pattern as TMAIL-204's push_platform fix (migration 061): sqlx
-- refuses the ENUM-to-String mismatch at decode time. Every read from
-- bulk_user_imports 500s with:
--     mismatched types; Rust type 'String' is not compatible with SQL
--     type 'bulk_import_status'
--
-- Widening to TEXT + CHECK keeps the value-set guarantee while letting
-- the existing model code work without sqlx custom-type plumbing.

-- Default still references the old ENUM type; drop it first so DROP TYPE
-- has nothing to complain about. Re-add as a TEXT default afterwards.
ALTER TABLE bulk_user_imports ALTER COLUMN status DROP DEFAULT;

ALTER TABLE bulk_user_imports
  ALTER COLUMN status TYPE TEXT USING status::text;

ALTER TABLE bulk_user_imports
  ALTER COLUMN status SET DEFAULT 'pending';

ALTER TABLE bulk_user_imports
  ADD CONSTRAINT bulk_user_imports_status_check
  CHECK (status IN ('pending', 'processing', 'completed', 'failed'));

DROP TYPE IF EXISTS bulk_import_status;
