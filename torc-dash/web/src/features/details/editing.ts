/**
 * Editing a workflow's resource requirements, Slurm schedulers, and per-job settings.
 *
 * Shared definitions are only edited while nothing is running, so running jobs and submitted
 * allocations never disagree with the records that describe them.
 */
import { api, unwrap, type Job, type ResourceRequirements } from "../../lib/api/client";

/** Why the workflow's shared definitions can't be edited now, or null if they can. */
export async function editBlockedReason(workflowId: number): Promise<string | null> {
  const jobs = (status: "running" | "pending") =>
    unwrap(api.GET("/jobs", { params: { query: { workflow_id: workflowId, status, limit: 1 } } })).then(
      (r) => r.total_count,
    );
  const nodes = (status: string) =>
    unwrap(
      api.GET("/scheduled_compute_nodes", {
        params: { query: { workflow_id: workflowId, status, limit: 1 } },
      }),
    ).then((r) => r.total_count);
  const [running, pending, active, queued] = await Promise.all([
    jobs("running"),
    jobs("pending"),
    nodes("active"),
    nodes("pending"),
  ]);
  if (running + pending > 0) return `${running + pending} job(s) are running or pending.`;
  if (active + queued > 0) return `${active + queued} Slurm allocation(s) are queued or active.`;
  return null;
}

/** Job statuses whose resources can't change: the job is (about to be) running with them. */
export const JOB_BUSY: ReadonlySet<string> = new Set(["running", "pending"]);

export type ResourceFields = Pick<
  ResourceRequirements,
  "num_cpus" | "memory" | "num_gpus" | "num_nodes" | "runtime"
>;

/**
 * Give one job new resource values. A requirement used only by this job is updated in place;
 * a shared one is cloned (as `<name>_job<id>`) so the other jobs keep theirs.
 */
export async function setJobResources(
  job: Job,
  current: ResourceRequirements | null,
  fields: ResourceFields,
): Promise<"updated" | "cloned"> {
  const workflow_id = job.workflow_id;
  if (current?.id != null && !(await sharedWithOthers(job, current.id))) {
    await unwrap(
      api.PUT("/resource_requirements/{id}", {
        params: { path: { id: current.id } },
        body: { ...current, ...fields },
      }),
    );
    return "updated";
  }
  const created = await unwrap(
    api.POST("/resource_requirements", {
      body: { workflow_id, name: `${current?.name ?? "resources"}_job${job.id}`, ...fields },
    }),
  );
  await unwrap(
    api.PUT("/jobs/{id}", {
      params: { path: { id: job.id! } },
      body: { ...job, resource_requirements_id: created.id },
    }),
  );
  return "cloned";
}

async function sharedWithOthers(job: Job, rrId: number): Promise<boolean> {
  return (await jobsUsing(job.workflow_id, rrId)) > 1;
}

/** How many jobs use a resource requirement. */
export async function jobsUsing(workflowId: number, rrId: number): Promise<number> {
  const r = await unwrap(
    api.GET("/jobs", {
      params: { query: { workflow_id: workflowId, resource_requirements_id: rrId, limit: 1 } },
    }),
  );
  return r.total_count;
}
