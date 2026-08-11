-- Opt a Slurm scheduler into serialized allocations: every allocation submitted for
-- this scheduler shares one Slurm job name and carries --dependency=singleton, so
-- Slurm runs them strictly one at a time. Used to chain allocations through a long
-- sequential workflow without a long-running process on the login node.
ALTER TABLE slurm_scheduler ADD COLUMN serialize_allocations INTEGER NOT NULL DEFAULT 0;
