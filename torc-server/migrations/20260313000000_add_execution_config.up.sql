-- Add execution_config column to workflow table.
-- This column stores execution configuration as a JSON blob (opaque to the server).
-- It controls execution mode (direct/slurm/auto) and related settings.
-- The server stores this without interpretation; only the client deserializes it.

ALTER TABLE workflow ADD COLUMN execution_config TEXT;
