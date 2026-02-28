-- Remove Slurm accounting (sacct) fields from the result table.
-- SQLite does not support DROP COLUMN on older versions; recreate the table.
CREATE TABLE result_without_sacct (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    workflow_id INTEGER NOT NULL,
    job_id INTEGER NOT NULL,
    run_id INTEGER NOT NULL,
    attempt_id INTEGER NOT NULL DEFAULT 1,
    compute_node_id INTEGER NOT NULL,
    return_code INTEGER NOT NULL,
    exec_time_minutes REAL NOT NULL,
    completion_time TEXT NOT NULL,
    status INTEGER NOT NULL,
    peak_memory_bytes INTEGER NULL,
    avg_memory_bytes INTEGER NULL,
    peak_cpu_percent REAL NULL,
    avg_cpu_percent REAL NULL,
    FOREIGN KEY (workflow_id) REFERENCES workflow(id) ON DELETE CASCADE,
    FOREIGN KEY (job_id) REFERENCES job(id) ON DELETE CASCADE,
    FOREIGN KEY (compute_node_id) REFERENCES compute_node(id) ON DELETE CASCADE
);

INSERT INTO result_without_sacct
    SELECT id, workflow_id, job_id, run_id, attempt_id, compute_node_id,
           return_code, exec_time_minutes, completion_time, status,
           peak_memory_bytes, avg_memory_bytes, peak_cpu_percent, avg_cpu_percent
    FROM result;

DROP TABLE result;
ALTER TABLE result_without_sacct RENAME TO result;
