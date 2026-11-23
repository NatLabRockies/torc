<!-- `torc workflows sync-status`: find orphaned Slurm jobs (dry run), then clean them up. -->
<script lang="ts">
  import { dashPost } from "../../lib/api/dash";
  import { errorText } from "../../lib/format";
  import { app } from "../../lib/state.svelte";
  import Modal from "../../lib/ui/Modal.svelte";
  import { toast } from "../../lib/ui/toast.svelte";
  import { wid } from "./actions";
  import Stats from "./Stats.svelte";

  type SyncData = {
    slurm_jobs_failed?: number;
    pending_allocations_cleaned?: number;
    running_jobs_failed?: number;
    failed_job_details?: {
      job_id?: number;
      job_name?: string;
      reason?: string;
      slurm_job_id?: string | number;
    }[];
  };
  type Response = { success: boolean; data?: SyncData; error?: string };

  let {
    workflowId,
    retry,
    onclose,
  }: {
    workflowId: number;
    /** Offered after cleanup, when this dialog was opened because (re)initialize was blocked. */
    retry?: { label: string; run: () => unknown };
    onclose: () => void;
  } = $props();

  let open = $state(true);
  let phase = $state<"loading" | "preview" | "applying" | "done" | "error">("loading");
  let data = $state<SyncData>({});
  let error = $state("");

  let slurmFailed = $derived(data.slurm_jobs_failed ?? 0);
  let pendingCleaned = $derived(data.pending_allocations_cleaned ?? 0);
  let runningFailed = $derived(data.running_jobs_failed ?? 0);
  let hasOrphans = $derived(slurmFailed + pendingCleaned + runningFailed > 0);

  async function call(dry_run: boolean): Promise<SyncData> {
    const res = await dashPost<Response>("/api/cli/sync-status", { ...wid(workflowId), dry_run });
    if (!res.success) throw new Error(res.error || "Sync status check failed");
    return res.data ?? {};
  }

  call(true)
    .then((d) => ((data = d), (phase = "preview")))
    .catch((e) => ((error = errorText(e)), (phase = "error")));

  async function apply() {
    phase = "applying";
    try {
      data = await call(false);
      phase = "done";
      toast.success("Sync status complete");
      app.refresh();
      if (!retry) setTimeout(() => (open = false), 3000);
    } catch (e) {
      error = `Sync failed: ${errorText(e)}`;
      phase = "error";
      toast.error("Sync status failed");
    }
  }

  const tone = (n: number, bad: "danger" | "warning") => (n > 0 ? bad : "success");
</script>

<Modal bind:open title="Sync status · workflow {workflowId}" size="xl" {onclose}>
  {#if phase === "loading" || phase === "applying"}
    <div class="empty">
      <span class="spinner"></span>
      {phase === "loading" ? "Checking Slurm job statuses…" : "Applying sync status cleanup…"}
    </div>
  {:else if phase === "error"}
    <p class="error-text">{error}</p>
  {:else if phase === "done"}
    <section class="done">
      <h4>Sync complete</h4>
      <ul>
        {#if slurmFailed}<li>{slurmFailed} Slurm job(s) marked as failed</li>{/if}
        {#if pendingCleaned}<li>{pendingCleaned} pending allocation(s) cleaned up</li>{/if}
        {#if runningFailed}<li>{runningFailed} running job(s) marked as failed</li>{/if}
      </ul>
    </section>
  {:else}
    <section>
      <h4>Summary</h4>
      <Stats
        items={[
          { label: "Slurm jobs failed", value: slurmFailed, tone: tone(slurmFailed, "danger") },
          {
            label: "Pending allocations cleaned",
            value: pendingCleaned,
            tone: tone(pendingCleaned, "warning"),
          },
          { label: "Running jobs failed", value: runningFailed, tone: tone(runningFailed, "danger") },
        ]}
      />
    </section>
    {#if data.failed_job_details?.length}
      <section>
        <h4>Affected jobs</h4>
        <table>
          <thead>
            <tr><th>Job ID</th><th>Job name</th><th>Reason</th><th>Slurm job ID</th></tr>
          </thead>
          <tbody>
            {#each data.failed_job_details as j, i (i)}
              <tr>
                <td><code>{j.job_id ?? "—"}</code></td>
                <td>{j.job_name || "—"}</td>
                <td>{j.reason || "—"}</td>
                <td><code>{j.slurm_job_id ?? "—"}</code></td>
              </tr>
            {/each}
          </tbody>
        </table>
      </section>
    {/if}
    {#if !hasOrphans}
      <p class="note">No orphaned jobs found. All running jobs have active Slurm allocations.</p>
    {/if}
  {/if}

  {#snippet footer()}
    <button class="btn" onclick={() => (open = false)}>
      {phase === "preview" && hasOrphans ? "Cancel" : "Close"}
    </button>
    {#if phase === "preview" && hasOrphans}
      <button class="btn btn-primary" onclick={apply}>Apply cleanup</button>
    {:else if phase === "done" && retry}
      <button class="btn btn-primary" onclick={() => (retry.run(), (open = false))}>
        {retry.label}
      </button>
    {/if}
  {/snippet}
</Modal>

<style>
  section + section,
  section + .note {
    margin-top: 20px;
  }
  h4 {
    margin-bottom: 8px;
    font-size: 13px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }
  th,
  td {
    padding: 6px 8px;
    border-bottom: 1px solid var(--border);
    text-align: left;
  }
  th {
    color: var(--text-muted);
    font-weight: 500;
  }
  .note {
    padding: 12px;
    border-radius: var(--radius);
    background: var(--surface-2);
    color: var(--text-muted);
  }
  .done h4 {
    color: var(--success);
  }
  .done ul {
    margin: 0;
    padding-left: 20px;
  }
</style>
