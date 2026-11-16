import type { Result } from "../../lib/api/client";

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
 * drive, UNC) are returned unchanged.
 */
export function resolveLogBaseDir(outputDir: string, submissionDir?: string | null): string {
  if (/^(\/|~|[A-Za-z]:[\\/]|\\\\)/.test(outputDir) || !submissionDir) return outputDir;
  return `${submissionDir.replace(/[\\/]+$/, "")}/${outputDir}`;
}

/** {output_dir}/job_stdio/job_wf{wf}_j{job}_r{run}_a{attempt}.o / .e */
export function stdioPaths(baseDir: string, r: Result): { stdout: string; stderr: string } {
  const base = `${baseDir}/job_stdio/job_wf${r.workflow_id}_j${r.job_id}_r${r.run_id}_a${r.attempt_id ?? 1}`;
  return { stdout: `${base}.o`, stderr: `${base}.e` };
}

export type ReadFileResponse = {
  success: boolean;
  content: string | null;
  error: string | null;
  is_json: boolean;
  exists: boolean;
};
