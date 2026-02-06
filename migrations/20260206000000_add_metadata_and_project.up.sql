-- Add project column to workflow table for grouping/categorization
ALTER TABLE workflow ADD COLUMN project TEXT NULL;

-- Add metadata column to workflow table for arbitrary JSON metadata
ALTER TABLE workflow ADD COLUMN metadata TEXT NULL;
