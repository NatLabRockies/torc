-- Persist the allocation end time (RFC3339) reported by the runner at registration.
-- Lets server-side/CLI diagnostics compute each active node's remaining walltime
-- without querying Slurm or depending on the live runner process.
ALTER TABLE compute_node ADD COLUMN end_time TEXT NULL;
