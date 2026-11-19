/** Loads a workflow's jobs/files/user data and turns them into cytoscape elements. */
import type { ElementDefinition } from "cytoscape";
import {
  api,
  listAll,
  unwrap,
  type FileRecord,
  type JobStatus,
  type Schemas,
  type UserData,
} from "../../lib/api/client";

type FileRel = Schemas["JobFileRelationshipModel"];
type UdRel = Schemas["JobUserDataRelationshipModel"];

export type GraphMode = "jobs" | "files" | "userdata";

export type GraphData = {
  elements: ElementDefinition[];
  nodeCount: number;
  /** Job count per status, for the legend. */
  statusCounts: Partial<Record<JobStatus, number>>;
};

const truncate = (s: string, max = 28) => (s.length > max ? `${s.slice(0, max - 1)}…` : s);

type Rel = { producer_job_id?: number | null; consumer_job_id?: number | null };

export async function loadGraph(workflowId: number, mode: GraphMode): Promise<GraphData> {
  const id = workflowId;
  const jobsP = listAll((offset, limit) =>
    unwrap(api.GET("/jobs", { params: { query: { workflow_id: id, offset, limit } } })),
  );
  const fileRelsP = () =>
    listAll((offset, limit) =>
      unwrap(
        api.GET("/workflows/{id}/job_file_relationships", {
          params: { path: { id }, query: { offset, limit } },
        }),
      ),
    );
  const udRelsP = () =>
    listAll((offset, limit) =>
      unwrap(
        api.GET("/workflows/{id}/job_user_data_relationships", {
          params: { path: { id }, query: { offset, limit } },
        }),
      ),
    );

  const elements: ElementDefinition[] = [];
  const statusCounts: Partial<Record<JobStatus, number>> = {};
  const jobNodes = (jobs: Awaited<typeof jobsP>) => {
    for (const job of jobs) {
      const status = job.status ?? "uninitialized";
      statusCounts[status] = (statusCounts[status] ?? 0) + 1;
      elements.push({
        group: "nodes",
        data: {
          id: `job-${job.id}`,
          label: truncate(job.name || `Job ${job.id}`, 40),
          name: job.name,
          kind: "job",
          jobId: job.id,
          status,
        },
        classes: "job",
      });
    }
  };
  const edge = (source: string, target: string, cls?: string) =>
    elements.push({
      group: "edges",
      data: { id: `${source}>${target}`, source, target },
      classes: cls,
    });

  if (mode === "jobs") {
    // Explicit dependencies plus implicit ones derived from file/user-data producer→consumer
    // links (those only land in job_dependencies once the workflow is initialized).
    const [jobs, deps, fileRels, udRels] = await Promise.all([
      jobsP,
      listAll((offset, limit) =>
        unwrap(
          api.GET("/workflows/{id}/job_dependencies", {
            params: { path: { id }, query: { offset, limit } },
          }),
        ),
      ),
      fileRelsP(),
      udRelsP(),
    ]);
    jobNodes(jobs);
    const seen = new Set<string>();
    const add = (from: number, to: number) => {
      const key = `${from}>${to}`;
      if (from === to || seen.has(key)) return;
      seen.add(key);
      edge(`job-${from}`, `job-${to}`);
    };
    for (const d of deps) add(d.depends_on_job_id, d.job_id);
    for (const r of [...fileRels, ...udRels] as Rel[]) {
      if (r.producer_job_id && r.consumer_job_id) add(r.producer_job_id, r.consumer_job_id);
    }
    return { elements, nodeCount: jobs.length, statusCounts };
  }

  const prefix = mode === "files" ? "file" : "ud";
  const itemsP: Promise<(FileRecord | UserData)[]> =
    mode === "files"
      ? listAll((offset, limit) =>
          unwrap(api.GET("/files", { params: { query: { workflow_id: id, offset, limit } } })),
        )
      : listAll((offset, limit) =>
          unwrap(api.GET("/user_data", { params: { query: { workflow_id: id, offset, limit } } })),
        );
  const relsP: Promise<(FileRel | UdRel)[]> = mode === "files" ? fileRelsP() : udRelsP();
  const [jobs, items, rels] = await Promise.all([jobsP, itemsP, relsP]);
  jobNodes(jobs);
  for (const item of items) {
    const name = item.name || `${mode === "files" ? "File" : "UserData"} ${item.id}`;
    elements.push({
      group: "nodes",
      data: {
        id: `${prefix}-${item.id}`,
        label: truncate(name),
        name,
        kind: mode === "files" ? "file" : "userdata",
        path: "path" in item ? item.path : undefined,
      },
      classes: mode === "files" ? "file" : "userdata",
    });
  }
  const seen = new Set<string>();
  for (const r of rels) {
    const itemId = "file_id" in r ? r.file_id : r.user_data_id;
    const node = `${prefix}-${itemId}`;
    const once = (s: string, t: string, cls: string) => {
      if (seen.has(`${s}>${t}`)) return;
      seen.add(`${s}>${t}`);
      edge(s, t, cls);
    };
    if (r.producer_job_id) once(`job-${r.producer_job_id}`, node, "output");
    if (r.consumer_job_id) once(node, `job-${r.consumer_job_id}`, "input");
  }
  return { elements, nodeCount: jobs.length + items.length, statusCounts };
}
