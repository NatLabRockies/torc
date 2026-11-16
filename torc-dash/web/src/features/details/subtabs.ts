import { api, listAll, unwrap, type Schemas } from "../../lib/api/client";

export type SubTab =
  | "jobs"
  | "running"
  | "results"
  | "events"
  | "files"
  | "user_data"
  | "resources"
  | "schedulers"
  | "compute_nodes"
  | "scheduled_nodes"
  | "slurm_stats";

export const SUBTABS: { id: SubTab; label: string }[] = [
  { id: "jobs", label: "Jobs" },
  { id: "running", label: "Running" },
  { id: "results", label: "Results" },
  { id: "events", label: "Events" },
  { id: "files", label: "Files" },
  { id: "user_data", label: "User Data" },
  { id: "resources", label: "Resources" },
  { id: "schedulers", label: "Schedulers" },
  { id: "compute_nodes", label: "Compute Nodes" },
  { id: "scheduled_nodes", label: "Scheduled Nodes" },
  { id: "slurm_stats", label: "Slurm Stats" },
];

export const QUICK_FILTERS: Partial<
  Record<SubTab, { label: string; filter: string; title: string }[]>
> = {
  jobs: [
    { label: "Failed", filter: "status=failed", title: "Show only failed jobs" },
    { label: "Running", filter: "status=running", title: "Show only running jobs" },
    { label: "Ready", filter: "status=ready", title: "Show only ready jobs" },
    { label: "Blocked", filter: "status=blocked", title: "Show only blocked jobs" },
  ],
  results: [
    { label: "Errors", filter: "return_code!=0", title: "Results with a non-zero return code" },
    { label: "Success", filter: "return_code=0", title: "Results with return code 0" },
    { label: "Failed", filter: "status=failed", title: "Failed results" },
  ],
  events: [{ label: "Errors", filter: "error", title: "Events mentioning errors" }],
};

export type SlurmStatRow = Schemas["SlurmStatsModel"] & { cpu_percent?: number };

const q = (workflow_id: number, offset: number, limit: number) => ({
  params: { query: { workflow_id, offset, limit } },
});

/** Fetch every row for one sub-tab. */
export async function loadRows(tab: SubTab, wf: number): Promise<object[]> {
  switch (tab) {
    case "jobs":
      return listAll((o, l) => unwrap(api.GET("/jobs", q(wf, o, l))));
    case "running":
      return listAll((o, l) =>
        unwrap(
          api.GET("/workflows/{id}/running_jobs", {
            params: { path: { id: wf }, query: { offset: o, limit: l } },
          }),
        ),
      );
    case "results":
      return listAll((o, l) => unwrap(api.GET("/results", q(wf, o, l))));
    case "events":
      return listAll((o, l) => unwrap(api.GET("/events", q(wf, o, l))));
    case "files":
      return listAll((o, l) => unwrap(api.GET("/files", q(wf, o, l))));
    case "user_data":
      return listAll((o, l) => unwrap(api.GET("/user_data", q(wf, o, l))));
    case "resources":
      return listAll((o, l) => unwrap(api.GET("/resource_requirements", q(wf, o, l))));
    case "schedulers":
      return listAll((o, l) => unwrap(api.GET("/slurm_schedulers", q(wf, o, l))));
    case "compute_nodes":
      return listAll((o, l) => unwrap(api.GET("/compute_nodes", q(wf, o, l))));
    case "scheduled_nodes":
      return listAll((o, l) => unwrap(api.GET("/scheduled_compute_nodes", q(wf, o, l))));
    case "slurm_stats":
      return loadSlurmStats(wf);
  }
}

/** Slurm stats enriched with CPU% = ave_cpu_seconds / exec time, joined from results. */
async function loadSlurmStats(wf: number): Promise<SlurmStatRow[]> {
  const [stats, results] = await Promise.all([
    listAll((o, l) => unwrap(api.GET("/slurm_stats", q(wf, o, l)))),
    listAll((o, l) =>
      unwrap(
        api.GET("/results", {
          params: { query: { workflow_id: wf, offset: o, limit: l, all_runs: true } },
        }),
      ),
    ).catch(() => []),
  ]);
  if (!stats.length) return stats;
  const execMinutes = new Map(
    results.map((r) => [`${r.job_id}_${r.run_id}_${r.attempt_id ?? 1}`, r.exec_time_minutes]),
  );
  return stats.map((s) => {
    const min = execMinutes.get(`${s.job_id}_${s.run_id}_${s.attempt_id}`);
    const cpu = s.ave_cpu_seconds && min ? (s.ave_cpu_seconds / (min * 60)) * 100 : undefined;
    return { ...s, cpu_percent: cpu };
  });
}

/** Row counts for the sub-tabs not covered by the workflow status (one tiny request each). */
export async function loadCounts(wf: number): Promise<Partial<Record<SubTab, number>>> {
  const one = q(wf, 0, 1);
  const calls: [SubTab, Promise<{ total_count: number }>][] = [
    ["results", unwrap(api.GET("/results", one))],
    ["events", unwrap(api.GET("/events", one))],
    ["files", unwrap(api.GET("/files", one))],
    ["user_data", unwrap(api.GET("/user_data", one))],
    ["resources", unwrap(api.GET("/resource_requirements", one))],
    ["schedulers", unwrap(api.GET("/slurm_schedulers", one))],
    ["compute_nodes", unwrap(api.GET("/compute_nodes", one))],
    ["scheduled_nodes", unwrap(api.GET("/scheduled_compute_nodes", one))],
    ["slurm_stats", unwrap(api.GET("/slurm_stats", one))],
  ];
  const settled = await Promise.allSettled(calls.map(([, p]) => p));
  const counts: Partial<Record<SubTab, number>> = {};
  settled.forEach((s, i) => {
    if (s.status === "fulfilled") counts[calls[i][0]] = s.value.total_count;
  });
  return counts;
}

/** Best one-line summary of an event's data. */
export function eventMessage(data: unknown): string {
  if (!data || typeof data !== "object") return data == null ? "" : String(data);
  const d = data as Record<string, unknown>;
  if (typeof d.message === "string") return d.message;
  if (d.job_name) return `Job: ${d.job_name}`;
  if (d.action) return d.user ? `${d.action} by ${d.user}` : String(d.action);
  if (d.category && d.type) return `${d.category}: ${d.type}`;
  return JSON.stringify(data);
}
