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

export async function dashFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    ...init,
    headers: { "Content-Type": "application/json", ...init?.headers },
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
