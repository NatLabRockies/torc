-- Add limit_resources column to workflow table.
-- When enabled (default), srun passes --mem and --cpus-per-task to enforce cgroup limits
-- for each job step when running inside a Slurm allocation. Set to 0 to allow jobs to
-- exceed their stated resource requirements (useful for exploratory workloads).
ALTER TABLE workflow ADD COLUMN limit_resources INTEGER NULL DEFAULT 1;
