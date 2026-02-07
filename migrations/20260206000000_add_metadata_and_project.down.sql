-- Remove project and metadata columns from workflow table
ALTER TABLE workflow DROP COLUMN project;
ALTER TABLE workflow DROP COLUMN metadata;
