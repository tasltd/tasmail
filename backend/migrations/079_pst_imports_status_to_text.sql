-- TMAIL-345: convert pst_imports.status from the pst_import_status
-- ENUM to TEXT + CHECK so it round-trips through models/pst_import.rs
-- (which decodes status as `String`).
--
-- Same pattern as the bulk_user_imports fix (migration 063) and the
-- push_platform fix (migration 061): sqlx refuses the ENUM-to-String
-- mismatch at decode time, so every POST /api/migration/pst/upload
-- 500s with "Failed to save PST file to disk" (actually a decode error
-- on the RETURNING * clause before the file is even written).
--
-- Widening to TEXT + CHECK keeps the value-set guarantee while letting
-- the existing handler code work without sqlx custom-type plumbing.

ALTER TABLE pst_imports ALTER COLUMN status DROP DEFAULT;

ALTER TABLE pst_imports
  ALTER COLUMN status TYPE TEXT USING status::text;

ALTER TABLE pst_imports
  ALTER COLUMN status SET DEFAULT 'pending';

ALTER TABLE pst_imports
  ADD CONSTRAINT pst_imports_status_check
  CHECK (status IN ('pending', 'processing', 'completed', 'failed'));

DROP TYPE IF EXISTS pst_import_status;
