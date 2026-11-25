-- Reverse 20261123000000_add_file_size_sha256. Plain column drops; no recreate.
ALTER TABLE file DROP COLUMN sha256;
ALTER TABLE file DROP COLUMN size;
