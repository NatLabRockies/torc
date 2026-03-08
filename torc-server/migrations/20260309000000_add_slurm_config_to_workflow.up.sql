-- Add srun_termination_signal and enable_cpu_bind columns, plus slurm_config
-- JSON blob for opaque Slurm configuration. The slurm_config column replaces
-- the pattern of adding individual Slurm-related columns — new Slurm settings
-- should be added to the JSON blob instead of as new columns.
ALTER TABLE workflow ADD COLUMN srun_termination_signal TEXT NULL;
ALTER TABLE workflow ADD COLUMN enable_cpu_bind INTEGER NULL;
ALTER TABLE workflow ADD COLUMN slurm_config TEXT NULL;
