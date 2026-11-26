-- Job-gated workflow action gates as rows, plus a per-action count of gates that do not yet
-- satisfy the trigger, kept current by a trigger on job.status. Polls check
-- `unsatisfied_count = 0` instead of scanning the gate list.
--
-- The count lives in its own small table: updating it on workflow_action would rewrite that row,
-- including its job_ids JSON (100+ KB for a large gate), on every gate status change.
--
-- Satisfied (status ints, pinned by a unit test in src/server/api/workflow_actions.rs):
-- - on_jobs_ready: status NOT IN (0, 1)            -- not uninitialized / blocked
-- - on_jobs_complete: status IN (5, 6, 7, 8, 9)    -- terminal or disabled
--
-- New tables and a trigger only; workflow_action is not changed (see CLAUDE.md migration warning).

CREATE TABLE workflow_action_job (
    action_id INTEGER NOT NULL REFERENCES workflow_action(id) ON DELETE CASCADE,
    job_id INTEGER NOT NULL REFERENCES job(id) ON DELETE CASCADE,
    PRIMARY KEY (action_id, job_id)
) WITHOUT ROWID;

CREATE INDEX idx_workflow_action_job_job_id ON workflow_action_job(job_id);

-- trigger_type is copied from workflow_action (immutable) so the job trigger reads one table.
CREATE TABLE workflow_action_gate (
    action_id INTEGER PRIMARY KEY REFERENCES workflow_action(id) ON DELETE CASCADE,
    trigger_type TEXT NOT NULL,
    unsatisfied_count INTEGER NOT NULL
);

-- Backfill existing job-gated actions. Gate ids that no longer exist get no row and count as
-- unsatisfied; an action with no gates gets 1, so neither ever triggers.
INSERT OR IGNORE INTO workflow_action_job (action_id, job_id)
SELECT a.id, j.id
FROM workflow_action a
JOIN json_each(a.job_ids) g
JOIN job j ON j.id = g.value
WHERE a.trigger_type IN ('on_jobs_ready', 'on_jobs_complete') AND a.job_ids IS NOT NULL;

INSERT INTO workflow_action_gate (action_id, trigger_type, unsatisfied_count)
SELECT a.id, a.trigger_type,
    MAX(COALESCE((SELECT COUNT(DISTINCT g.value) FROM json_each(a.job_ids) g), 0), 1)
    - (SELECT COUNT(*) FROM workflow_action_job l JOIN job j ON j.id = l.job_id
       WHERE l.action_id = a.id
         AND CASE a.trigger_type WHEN 'on_jobs_ready' THEN j.status NOT IN (0, 1)
             ELSE j.status IN (5, 6, 7, 8, 9) END)
FROM workflow_action a
WHERE a.trigger_type IN ('on_jobs_ready', 'on_jobs_complete');

-- Adjust the gating actions' counts by the old/new status difference, in the transaction that
-- changes the status. The WHEN guard is one index probe, so jobs that gate no action pay almost
-- nothing; actions whose satisfied-ness did not change are skipped.
CREATE TRIGGER job_status_updates_action_gates
AFTER UPDATE OF status ON job
WHEN OLD.status IS NOT NEW.status
    AND EXISTS (SELECT 1 FROM workflow_action_job WHERE job_id = NEW.id)
BEGIN
    UPDATE workflow_action_gate
    SET unsatisfied_count = unsatisfied_count + CASE trigger_type
        WHEN 'on_jobs_ready' THEN (NEW.status IN (0, 1)) - (OLD.status IN (0, 1))
        ELSE (NEW.status NOT IN (5, 6, 7, 8, 9)) - (OLD.status NOT IN (5, 6, 7, 8, 9)) END
    FROM workflow_action_job l
    WHERE l.job_id = NEW.id AND workflow_action_gate.action_id = l.action_id
      AND CASE trigger_type
        WHEN 'on_jobs_ready' THEN (NEW.status IN (0, 1)) != (OLD.status IN (0, 1))
        ELSE (NEW.status NOT IN (5, 6, 7, 8, 9)) != (OLD.status NOT IN (5, 6, 7, 8, 9)) END;
END;
