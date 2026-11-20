import type { Job, Result, Schemas, Workflow } from "../../lib/api/client";

/** Shared with the job details modal (legacy key), so both remember the same directory. */
export const OUTPUT_DIR_KEY = "torc-job-logs-output-dir";

export function loadOutputDir(): string {
  try {
    return localStorage.getItem(OUTPUT_DIR_KEY) || "torc_output";
  } catch {
    return "torc_output";
  }
}

export function saveOutputDir(dir: string) {
  try {
    localStorage.setItem(OUTPUT_DIR_KEY, dir);
  } catch {
    // Convenience only.
  }
}

/**
 * Join a relative output dir to the workflow's submission directory (where logs are written) so
 * paths resolve regardless of torc-dash's working directory. Absolute paths (POSIX, `~`, Windows
 * drive, UNC) are returned unchanged; the torc-dash backend expands `~`. Only `~/` counts as home:
 * the browser can't know the server's OS, and on Unix `~\` is an ordinary relative name.
 */
export function resolveLogBaseDir(outputDir: string, submissionDir?: string | null): string {
  if (/^(\/|~(\/|$)|[A-Za-z]:[\\/]|\\\\)/.test(outputDir) || !submissionDir) return outputDir;
  return `${submissionDir.replace(/[\\/]+$/, "")}/${outputDir}`;
}

/** {output_dir}/job_stdio/job_wf{wf}_j{job}_r{run}_a{attempt}.o / .e */
export function stdioPaths(
  baseDir: string,
  r: Pick<Result, "workflow_id" | "job_id" | "run_id" | "attempt_id">,
): { stdout: string; stderr: string; combined: string } {
  const base = `${baseDir}/job_stdio/job_wf${r.workflow_id}_j${r.job_id}_r${r.run_id}_a${r.attempt_id ?? 1}`;
  return { stdout: `${base}.o`, stderr: `${base}.e`, combined: `${base}.log` };
}

/** Mirrors `ExecutionConfig::stdio_for_job`: a per-job override replaces the workflow default. */
export function stdioMode(wf: Workflow | null, jobName: string): Schemas["StdioMode"] {
  const ec = wf?.execution_config;
  return (ec?.job_stdio_overrides?.[jobName] ?? ec?.stdio)?.mode ?? "separate";
}

export type LiveFile = {
  stream: "stdout" | "stderr" | "combined";
  path: string;
};

/** The files a job's current attempt writes, given its stdio mode (empty for mode `none`). */
export function liveFiles(baseDir: string, wf: Workflow, job: Job): LiveFile[] {
  const p = stdioPaths(baseDir, {
    workflow_id: wf.id!,
    job_id: job.id!,
    run_id: wf.run_id ?? 1,
    attempt_id: job.attempt_id,
  });
  const out: LiveFile[] = [];
  const mode = stdioMode(wf, job.name);
  if (mode === "combined") out.push({ stream: "combined", path: p.combined });
  if (mode === "separate" || mode === "no_stderr") out.push({ stream: "stdout", path: p.stdout });
  if (mode === "separate" || mode === "no_stdout") out.push({ stream: "stderr", path: p.stderr });
  return out;
}

export type TailFileResponse = {
  exists: boolean;
  content: string;
  offset: number;
  error: string | null;
};

export type ReadFileResponse = {
  success: boolean;
  content: string | null;
  error: string | null;
  is_json: boolean;
  exists: boolean;
};
