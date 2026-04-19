ALTER TABLE compute_node ADD COLUMN sample_count INTEGER NULL;
ALTER TABLE compute_node ADD COLUMN peak_cpu_percent REAL NULL;
ALTER TABLE compute_node ADD COLUMN avg_cpu_percent REAL NULL;
ALTER TABLE compute_node ADD COLUMN peak_memory_bytes INTEGER NULL;
ALTER TABLE compute_node ADD COLUMN avg_memory_bytes INTEGER NULL;
