/** Workflow operations that need no dedicated dialog (they use confirm() and toasts). */
import { api, unwrap } from "../../lib/api/client";
import { cli, dashPost, download, op, type CliResponse } from "../../lib/api/dash";
import { errorText } from "../../lib/format";
import { app } from "../../lib/state.svelte";
import { follow } from "../notify/notify.svelte";
import { confirm } from "../../lib/ui/confirm.svelte";
import { toast } from "../../lib/ui/toast.svelte";
import { loadOutputDir, resolveLogBaseDir } from "../debugging/logs";

export const wid = (id: number) => ({ workflow_id: String(id) });

/** Run a CLI action, toast the outcome, and refresh on success. */
async function act(what: string, call: () => Promise<unknown>, done: string) {
  try {
    await call();
    toast.success(done);
    app.refresh();
  } catch (e) {
    toast.error(`${what} failed: ${errorText(e)}`);
  }
}

export async function submitWorkflow(id: number) {
  const ok = await confirm({
    title: `Submit workflow ${id}?`,
    message: "Submit this workflow to the scheduler (e.g., Slurm).",
    confirmLabel: "Submit",
  });
  if (!ok) return;
  follow(id);
  toast.info(`Submitting workflow ${id}…`);
  await act("Submit", () => cli("submit", wid(id)), `Workflow ${id} submitted`);
}

/** Server refusals for a reset while jobs or Slurm allocations still look active. */
const ACTIVE_WORK = /jobs are currently running or pending|compute nodes are currently pending or active/;

/**
 * Run initialize/reinitialize. If it fails only because input files are missing, offer to
 * retry with `--force`, which skips that check (as the CLI does). If it fails because jobs or
 * allocations still look active (e.g., after `scancel`), returns "needs-sync" when the user
 * chooses to check Slurm status. The file check runs before the active-work guard, so the
 * forced retry can hit that guard too; both attempts share one error handler.
 */
async function runInit(
  cmd: "initialize" | "reinitialize",
  label: string,
  id: number,
): Promise<"needs-sync" | void> {
  for (const force of [false, true]) {
    try {
      await cli(cmd, { ...wid(id), force });
      toast.success(`Workflow ${id} ${cmd}d`);
      app.refresh();
      return;
    } catch (e) {
      const msg = errorText(e);
      if (ACTIVE_WORK.test(msg)) {
        const ok = await confirm({
          title: "Workflow still has active jobs",
          message:
            `Workflow ${id} has jobs marked running or Slurm allocations marked active, so it ` +
            `can't be ${cmd}d yet.\n\nIf those Slurm jobs were canceled (scancel), timed out, or ` +
            `died, check Slurm status to mark the orphaned jobs as failed, then ${label.toLowerCase()}.`,
          confirmLabel: "Check Slurm status…",
        });
        return ok ? "needs-sync" : undefined;
      }
      if (force || !msg.includes("Missing required files")) {
        return toast.error(`${label} failed: ${msg}`);
      }
      const ok = await confirm({
        title: "Required input files are missing",
        message: `${msg}\n\n${label} anyway? Jobs that need these files will fail unless they appear before the jobs run.`,
        confirmLabel: `${label} anyway`,
        danger: true,
      });
      if (!ok) return;
    }
  }
}

/** Initialize. If output files from an earlier run exist, say so first: initializing keeps them. */
export async function initializeWorkflow(id: number) {
  try {
    const check = await dashPost<CliResponse>("/api/cli/check-initialize", wid(id));
    const dry = check.success && check.stdout ? JSON.parse(check.stdout) : null;
    const count: number = dry?.existing_output_file_count ?? 0;
    if (count > 0) {
      const files: string[] = dry.existing_output_files ?? [];
      const list = files.slice(0, 10).map((f) => `  • ${f}`);
      if (files.length > 10) list.push(`  … and ${files.length - 10} more`);
      const ok = await confirm({
        title: "Output files already exist",
        message: `Workflow ${id} has ${count} output file(s) from an earlier run. Initializing does not delete them:\n\n${list.join("\n")}`,
        confirmLabel: "Initialize",
      });
      if (!ok) return;
    }
  } catch {
    // The dry run is advisory; fall through to a normal initialize like the CLI would.
  }
  return runInit("initialize", "Initialize", id);
}

export const reinitializeWorkflow = (id: number) => runInit("reinitialize", "Reinitialize", id);

export async function resetWorkflow(id: number) {
  const ok = await confirm({
    title: `Reset workflow ${id}?`,
    message: "This sets all jobs back to the uninitialized state.",
    confirmLabel: "Reset status",
    danger: true,
  });
  if (ok) await act("Reset", () => cli("reset-status", wid(id)), `Workflow ${id} status reset`);
}

export async function cancelWorkflow(id: number) {
  const ok = await confirm({
    title: `Cancel workflow ${id}?`,
    message: "This will terminate all running Slurm jobs.",
    confirmLabel: "Cancel workflow",
    danger: true,
  });
  if (ok) await act("Cancel", () => cli("cancel", wid(id)), `Workflow ${id} canceled`);
}

/** Delete workflows after confirmation. Returns the IDs that were deleted. */
export async function deleteWorkflows(ids: number[]): Promise<number[]> {
  if (ids.length === 0) return [];
  const noun = ids.length === 1 ? `workflow ${ids[0]}` : `${ids.length} workflows`;
  const ok = await confirm({
    title: `Delete ${noun}?`,
    message: "This action cannot be undone.",
    confirmLabel: "Delete",
    danger: true,
  });
  if (!ok) return [];

  const results = await Promise.allSettled(ids.map((id) => cli("delete", wid(id))));
  const deleted = ids.filter((_, i) => results[i].status === "fulfilled");
  const failed = results.flatMap((r, i) =>
    r.status === "rejected" ? [`${ids[i]}: ${errorText(r.reason)}`] : [],
  );
  if (deleted.length) {
    toast.success(deleted.length === 1 ? `Deleted workflow ${deleted[0]}` : `Deleted ${deleted.length} workflows`);
  }
  if (failed.length) toast.error(`Failed to delete ${failed.join("; ")}`);
  if (app.workflowId !== null && deleted.includes(app.workflowId)) app.selectWorkflow(null);
  app.refresh();
  return deleted;
}

/** Archive or unarchive workflows. Archived workflows are hidden unless "Show archived" is on. */
export async function archiveWorkflows(ids: number[], archived: boolean) {
  if (ids.length === 0) return;
  const noun = ids.length === 1 ? `Workflow ${ids[0]}` : `${ids.length} workflows`;
  await act(
    archived ? "Archive" : "Unarchive",
    () => op("archive", { workflow_ids: ids, archived }),
    `${noun} ${archived ? "archived" : "unarchived"}`,
  );
}

/**
 * Download a .tar.gz of the workflow's logs (torc logs bundle), from the output dir the
 * Debugging tab uses, resolved against the workflow's submission directory.
 */
export async function downloadLogs(id: number) {
  let submissionDir: string | null = null;
  try {
    const wf = await unwrap(api.GET("/workflows/{id}", { params: { path: { id } } }));
    submissionDir = wf.submission_directory ?? null;
  } catch {
    // Fall back to torc-dash's working directory, like the Debugging tab.
  }
  const outputDir = resolveLogBaseDir(loadOutputDir(), submissionDir);
  toast.info(`Bundling logs from ${outputDir}…`);
  download("/api/download/logs", { workflow_id: id, output_dir: outputDir });
}

/** Stop a `torc run` that the dashboard started, even if its output panel was closed. */
export async function stopLocalRun(id: number) {
  try {
    const res = await dashPost<{ stopped: boolean }>("/api/cli/stop-run", wid(id));
    if (res.stopped) toast.success(`Stopping the local run of workflow ${id}`);
    else toast.info(`No local run of workflow ${id} started from this dashboard is active`);
    app.refresh();
  } catch (e) {
    toast.error(`Stop failed: ${errorText(e)}`);
  }
}
