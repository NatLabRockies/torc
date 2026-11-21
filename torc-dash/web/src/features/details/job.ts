import {
  api,
  listAll,
  unwrap,
  type FileRecord,
  type Job,
  type ResourceRequirements,
  type Result,
  type UserData,
  type Workflow,
} from "../../lib/api/client";

export type JobDetails = {
  job: Job;
  workflow: Workflow | null;
  results: Result[];
  resourceReq: ResourceRequirements | null;
  inputFiles: FileRecord[];
  outputFiles: FileRecord[];
  inputUserData: UserData[];
  outputUserData: UserData[];
  blockedBy: Job[];
  blocks: Job[];
};

/** Up to this many ids are fetched one by one; beyond that, list the workflow and filter. */
const MAX_INDIVIDUAL = 50;

async function byIds<T extends { id?: number | null }>(
  ids: number[] | null | undefined,
  one: (id: number) => Promise<T>,
  all: () => Promise<T[]>,
): Promise<T[]> {
  if (!ids?.length) return [];
  if (ids.length <= MAX_INDIVIDUAL) return Promise.all(ids.map(one));
  const wanted = new Set(ids);
  return (await all()).filter((x) => wanted.has(x.id!));
}

/** Ids per `/jobs?ids=` request: keeps the URL well under server limits on large fan-ins. */
const IDS_PER_REQUEST = 1000;

async function jobsByIds(workflow_id: number, ids: number[] | null | undefined): Promise<Job[]> {
  const jobs: Job[] = [];
  for (let i = 0; i < (ids?.length ?? 0); i += IDS_PER_REQUEST) {
    const chunk = ids!.slice(i, i + IDS_PER_REQUEST);
    const query = (o: number, l: number) => ({ workflow_id, offset: o, limit: l, ids: chunk });
    jobs.push(
      ...(await listAll((o, l) => unwrap(api.GET("/jobs", { params: { query: query(o, l) } })))),
    );
  }
  return jobs;
}

/**
 * Load one job and everything the details modal shows, using per-job queries so this stays fast
 * on workflows with 100k jobs. Only needs the job id; the workflow comes from the job.
 */
export async function loadJobDetails(id: number): Promise<JobDetails> {
  const job = await unwrap(api.GET("/jobs/{id}", { params: { path: { id } } }));
  const wf = job.workflow_id;
  const page = (offset: number, limit: number) => ({ workflow_id: wf, offset, limit });

  const [
    workflow,
    results,
    resourceReq,
    inputFiles,
    outputFiles,
    inputUserData,
    outputUserData,
    blockedBy,
    blocks,
  ] = await Promise.all([
    unwrap(api.GET("/workflows/{id}", { params: { path: { id: wf } } })).catch(() => null),
    listAll((o, l) =>
      unwrap(api.GET("/results", { params: { query: { ...page(o, l), job_id: id, all_runs: true } } })),
    ),
    job.resource_requirements_id
      ? unwrap(
          api.GET("/resource_requirements/{id}", {
            params: { path: { id: job.resource_requirements_id } },
          }),
        ).catch(() => null)
      : null,
    byIds(
      job.input_file_ids,
      (fid) => unwrap(api.GET("/files/{id}", { params: { path: { id: fid } } })),
      () => listAll((o, l) => unwrap(api.GET("/files", { params: { query: page(o, l) } }))),
    ),
    listAll((o, l) =>
      unwrap(api.GET("/files", { params: { query: { ...page(o, l), produced_by_job_id: id } } })),
    ),
    listAll((o, l) =>
      unwrap(api.GET("/user_data", { params: { query: { ...page(o, l), consumer_job_id: id } } })),
    ),
    listAll((o, l) =>
      unwrap(api.GET("/user_data", { params: { query: { ...page(o, l), producer_job_id: id } } })),
    ),
    jobsByIds(wf, job.depends_on_job_ids),
    listAll((o, l) =>
      unwrap(api.GET("/jobs", { params: { query: { ...page(o, l), upstream_job_id: id } } })),
    ),
  ]);

  results.sort((a, b) => a.run_id - b.run_id || (a.attempt_id ?? 1) - (b.attempt_id ?? 1));

  return {
    job,
    workflow,
    results,
    resourceReq,
    inputFiles,
    outputFiles,
    inputUserData,
    outputUserData,
    blockedBy,
    blocks,
  };
}
