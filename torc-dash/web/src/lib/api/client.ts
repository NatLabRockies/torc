/**
 * Typed client for the torc-server REST API, proxied by torc-dash at /torc-service/v1.
 * Types come from api/openapi.yaml via `npm run gen:api` (schema.d.ts is generated; don't edit).
 *
 * Usage:
 *   const wf = await unwrap(api.GET("/workflows/{id}", { params: { path: { id } } }));
 *   const jobs = await listAll((offset, limit) =>
 *     unwrap(api.GET("/jobs", { params: { query: { workflow_id: id, offset, limit } } })));
 */
import createClient from "openapi-fetch";
import type { components, paths } from "./schema";
import { tokenHeaders } from "./token";

export type Schemas = components["schemas"];
export type Workflow = Schemas["WorkflowModel"];
export type Job = Schemas["JobModel"];
export type JobStatus = Schemas["JobStatus"];
export type FileRecord = Schemas["FileModel"];
export type UserData = Schemas["UserDataModel"];
export type Result = Schemas["ResultModel"];
export type TorcEvent = Schemas["EventModel"];
export type ComputeNode = Schemas["ComputeNodeModel"];
export type ResourceRequirements = Schemas["ResourceRequirementsModel"];
export type ScheduledComputeNode = Schemas["ScheduledComputeNodesModel"];
export type SlurmScheduler = Schemas["SlurmSchedulerModel"];

export const API_BASE = "/torc-service/v1";
// Array query params (`ids`, multi-value `status`) are comma-separated (`explode: false`).
export const api = createClient<paths>({
  baseUrl: API_BASE,
  headers: tokenHeaders,
  querySerializer: { array: { style: "form", explode: false } },
});

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
  }
}

/** Turn an openapi-fetch result into its data, or throw an ApiError with the server's message. */
export async function unwrap<T>(
  call: Promise<{ data?: T; error?: unknown; response: Response }>,
): Promise<T> {
  const { data, error, response } = await call;
  if (error !== undefined || !response.ok) {
    throw new ApiError(response.status, errorMessage(error) || `HTTP ${response.status}`);
  }
  return data as T;
}

function errorMessage(error: unknown): string {
  if (!error) return "";
  if (typeof error === "string") return error;
  const e = error as Record<string, unknown>;
  for (const key of ["message", "error", "detail"]) {
    const v = e[key];
    if (typeof v === "string") return v;
    if (v && typeof v === "object") return JSON.stringify(v);
  }
  return JSON.stringify(error);
}

type Page<T> = { items: T[]; has_more: boolean };

/** Fetch every page of a paginated list endpoint. */
export async function listAll<T>(
  fetchPage: (offset: number, limit: number) => Promise<Page<T>>,
  limit = 10_000,
): Promise<T[]> {
  const all: T[] = [];
  for (let offset = 0; ; ) {
    const page = await fetchPage(offset, limit);
    all.push(...page.items);
    if (!page.has_more || page.items.length === 0) return all;
    offset += page.items.length;
  }
}
