/**
 * Calls to torc-dash's own endpoints (/api/*): CLI wrappers, server management, chat, etc.
 * These are not in the OpenAPI spec; declare request/response types next to the feature using them.
 */

/** Response shape shared by most /api/cli/* endpoints. */
export type CliResponse = {
  success: boolean;
  stdout: string;
  stderr: string;
  exit_code: number | null;
};

import { tokenHeaders, withToken } from "./token";

export async function dashFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    ...init,
    headers: { "Content-Type": "application/json", ...tokenHeaders, ...init?.headers },
  });
  const text = await response.text();
  if (!response.ok) throw new Error(text || `HTTP ${response.status}`);
  return (text ? JSON.parse(text) : null) as T;
}

export const dashGet = <T>(path: string) => dashFetch<T>(path);

export const dashPost = <T>(path: string, body: unknown) =>
  dashFetch<T>(path, { method: "POST", body: JSON.stringify(body) });

/** POST /api/cli/<endpoint>. Throws if the command reports failure, with stderr as the message. */
export async function cli(endpoint: string, body: unknown): Promise<CliResponse> {
  const res = await dashPost<CliResponse>(`/api/cli/${endpoint}`, body);
  if (!res.success) throw new Error(res.stderr.trim() || res.stdout.trim() || "Command failed");
  return res;
}

/** Envelope returned by /api/cli/op and the JSON CLI wrappers (recover, sync-status). */
export type OpResponse<T> = {
  success: boolean;
  data: T | null;
  stdout?: string;
  stderr?: string;
  error?: string;
};

/**
 * Run a whitelisted torc command via POST /api/cli/op (see `build_op` in torc-dash.rs) and
 * return its parsed JSON output. Throws with the command's error on failure.
 */
export async function op<T>(name: string, params: Record<string, unknown> = {}): Promise<T> {
  const res = await dashPost<OpResponse<T>>("/api/cli/op", { op: name, ...params });
  if (!res.success) throw new Error(res.error || "Command failed");
  return res.data as T;
}

/** Start a browser download of a torc-dash /api/download/* URL. */
export function download(path: string, params: Record<string, string | number | boolean>) {
  const query = new URLSearchParams(
    Object.entries(params).map(([k, v]) => [k, String(v)]),
  );
  // A unique URL per download: the desktop app tracks downloads by URL, and macOS reports nothing else.
  // (Not crypto.randomUUID(): browsers allow it only over HTTPS or on localhost.)
  query.set("_", Math.random().toString(36).slice(2));
  const a = document.createElement("a");
  a.href = withToken(`${path}?${query}`);
  a.download = "";
  document.body.append(a);
  a.click();
  a.remove();
}
