-- Size in bytes and SHA-256 (lowercase hex) of a file, recorded together with st_mtime.
-- Set only for workflows with enable_ro_crate. NULL otherwise.
ALTER TABLE file ADD COLUMN size INTEGER NULL;
ALTER TABLE file ADD COLUMN sha256 TEXT NULL;
