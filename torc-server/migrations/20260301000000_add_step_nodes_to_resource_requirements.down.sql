-- SQLite does not support DROP COLUMN in older versions; recreate the table without step_nodes.
CREATE TABLE resource_requirements_new (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    workflow_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    num_cpus INTEGER NOT NULL DEFAULT 1,
    num_gpus INTEGER NOT NULL DEFAULT 0,
    num_nodes INTEGER NOT NULL DEFAULT 1,
    memory TEXT NOT NULL DEFAULT '1m',
    runtime TEXT NOT NULL DEFAULT 'P0DT1M',
    memory_bytes INTEGER NOT NULL,
    runtime_s INTEGER NOT NULL,
    FOREIGN KEY (workflow_id) REFERENCES workflow(id) ON DELETE CASCADE
);

INSERT INTO resource_requirements_new
    SELECT id, workflow_id, name, num_cpus, num_gpus, num_nodes, memory, runtime,
           memory_bytes, runtime_s
    FROM resource_requirements;

DROP TABLE resource_requirements;
ALTER TABLE resource_requirements_new RENAME TO resource_requirements;

-- Recreate index that was dropped with the old table.
CREATE INDEX idx_resource_requirements_workflow_id ON resource_requirements(workflow_id);
