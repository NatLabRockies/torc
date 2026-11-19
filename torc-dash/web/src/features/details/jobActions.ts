/** Job-level actions shared by the Jobs table selection and the job details dialog. */
import { cli } from "../../lib/api/dash";
import { errorText } from "../../lib/format";
import { app } from "../../lib/state.svelte";
import { confirm } from "../../lib/ui/confirm.svelte";
import { toast } from "../../lib/ui/toast.svelte";

/**
 * `torc jobs reset-status --reinit`: reset the jobs to uninitialized and reinitialize the workflow,
 * which also resets their downstream jobs. Returns whether it ran.
 */
export async function rerunJobs(ids: number[]): Promise<boolean> {
  const what = ids.length === 1 ? `job ${ids[0]}` : `${ids.length} jobs`;
  const ok = await confirm({
    title: `Reset ${what} for rerun?`,
    message:
      "Resets the selected jobs to uninitialized and reinitializes the workflow, which also " +
      "resets the jobs that depend on them. Then run or submit the workflow to execute them.",
    confirmLabel: "Reset for rerun",
  });
  if (!ok) return false;
  try {
    await cli("reset-jobs", { job_ids: ids });
    toast.success(`Reset ${what} for rerun`);
    app.refresh();
    return true;
  } catch (e) {
    toast.error(`Reset failed: ${errorText(e)}`);
    return false;
  }
}
