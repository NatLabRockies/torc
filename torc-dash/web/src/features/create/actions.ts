/** torc-dash endpoints used by the create-workflow modal. Shapes match src/bin/torc-dash.rs. */
import { cli, dashGet, dashPost, type CliResponse } from "../../lib/api/dash";
import { confirm } from "../../lib/ui/confirm.svelte";
import { toast } from "../../lib/ui/toast.svelte";

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

/** Initialize a workflow, asking before deleting existing output files (as the legacy UI did). */
export async function initializeWorkflow(workflowId: number): Promise<boolean> {
  const workflow_id = String(workflowId);
  let force = false;
  const check = await dashPost<CliResponse>("/api/cli/check-initialize", { workflow_id });
  if (check.success) {
    try {
      const dry = JSON.parse(check.stdout) as {
        existing_output_file_count?: number;
        existing_output_files?: string[];
      };
      const count = dry.existing_output_file_count ?? 0;
      if (count > 0) {
        const files = dry.existing_output_files ?? [];
        const list = files.slice(0, 10).join("\n");
        const more = files.length > 10 ? `\n… and ${files.length - 10} more` : "";
        const ok = await confirm({
          title: "Delete existing output files?",
          message: `Workflow ${workflowId} has ${count} existing output file(s) that initialization will delete:\n\n${list}${more}`,
          confirmLabel: "Delete and initialize",
          danger: true,
        });
        if (!ok) return false;
        force = true;
      }
    } catch {
      // Unparseable dry run: proceed like the legacy UI.
    }
  }
  await cli("initialize", { workflow_id, force });
  toast.success(`Workflow ${workflowId} initialized`);
  return true;
}
