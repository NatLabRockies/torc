/** torc-dash endpoints used by the create-workflow modal. Shapes match src/bin/torc-dash.rs. */
import { cli, dashGet, dashPost, type CliResponse } from "../../lib/api/dash";

/** Body of /api/cli/create and /api/cli/validate. */
export type SpecBody = { spec: string; is_file: boolean; file_extension?: string };

export type HpcProfiles = {
  success: boolean;
  profiles: { name: string; display_name: string; description: string; is_detected: boolean }[];
  detected_profile: string | null;
  error: string | null;
};

export const getHpcProfiles = () => dashGet<HpcProfiles>("/api/cli/hpc-profiles");

/** Parsed stdout of `torc -f json create --dry-run`. */
export type ValidationReport = {
  valid: boolean;
  errors: string[];
  warnings: string[];
  summary?: {
    workflow_name?: string;
    job_count?: number;
    job_count_before_expansion?: number;
    file_count?: number;
    user_data_count?: number;
    resource_requirements_count?: number;
    slurm_scheduler_count?: number;
    action_count?: number;
    has_schedule_nodes_action?: boolean;
  };
};

/** Dry-run create. Never throws for an invalid spec; returns a report with errors instead. */
export async function validateSpec(body: SpecBody): Promise<ValidationReport> {
  const res = await dashPost<CliResponse>("/api/cli/validate", body);
  try {
    const report = JSON.parse(res.stdout) as ValidationReport;
    return { ...report, errors: report.errors ?? [], warnings: report.warnings ?? [] };
  } catch {
    const msg = res.stderr.trim() || res.stdout.trim() || "Validation failed";
    return { valid: res.success, errors: res.success ? [] : [msg], warnings: [] };
  }
}

export function extractWorkflowId(stdout: string): number | null {
  try {
    const id = Number(JSON.parse(stdout).workflow_id);
    if (Number.isFinite(id)) return id;
  } catch {
    // Not JSON; fall through to text patterns.
  }
  const m = stdout.match(/Created workflow\s+(\d+)/i) ?? stdout.match(/ID:\s*(\d+)/);
  return m ? Number(m[1]) : null;
}

export async function createWorkflow(
  body: SpecBody,
  slurm: { account: string; profile: string | null } | null,
): Promise<number | null> {
  const res = slurm
    ? await cli("create-slurm", { ...body, account: slurm.account, profile: slurm.profile ?? undefined })
    : await cli("create", body);
  return extractWorkflowId(res.stdout);
}

