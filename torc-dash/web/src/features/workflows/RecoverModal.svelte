<!-- `torc recover`: dry-run preview, then apply. -->
<script lang="ts">
  import { dashPost } from "../../lib/api/dash";
  import { errorText } from "../../lib/format";
  import { app } from "../../lib/state.svelte";
  import Modal from "../../lib/ui/Modal.svelte";
  import { toast } from "../../lib/ui/toast.svelte";
  import { wid } from "./actions";
  import Stats from "./Stats.svelte";

  type Adjustment = {
    job_names: string[];
    memory_adjusted?: boolean;
    original_memory?: string;
    new_memory?: string;
    runtime_adjusted?: boolean;
    original_runtime?: string;
    new_runtime?: string;
  };
  type PlannedScheduler = {
    name: string;
    job_count: number;
    num_allocations: number;
    account?: string;
    partition?: string;
    walltime?: string;
    mem?: string;
    nodes?: number;
  };
  type FailedJob = {
    job_name: string;
    return_code: number;
    likely_oom?: boolean;
    oom_reason?: string;
    peak_memory_formatted?: string;
    likely_timeout?: boolean;
    timeout_reason?: string;
  };
  type RecoverResult = {
    jobs_to_retry?: unknown[];
    oom_fixed?: number;
    timeout_fixed?: number;
    other_failures?: number;
    unknown_retried?: number;
    adjustments?: Adjustment[];
    slurm_dry_run?: { planned_schedulers?: PlannedScheduler[]; total_allocations?: number };
  };
  type RecoverData = { result: RecoverResult; diagnosis?: { failed_jobs?: FailedJob[] } };
  type Response = { success: boolean; data?: RecoverData; error?: string };

  let { workflowId, onclose }: { workflowId: number; onclose: () => void } = $props();

  let open = $state(true);
  let phase = $state<"loading" | "preview" | "applying" | "done" | "error">("loading");
  let data = $state<RecoverData | null>(null);
  let error = $state("");

  let r = $derived(data?.result);
  let retryCount = $derived(r?.jobs_to_retry?.length ?? 0);

  async function call(dry_run: boolean): Promise<RecoverData> {
    const res = await dashPost<Response>("/api/cli/recover", { ...wid(workflowId), dry_run });
    if (!res.success) throw new Error(res.error || "Recovery check failed");
    if (!res.data?.result) throw new Error("No recovery data available.");
    return res.data;
  }

  call(true)
    .then((d) => ((data = d), (phase = "preview")))
    .catch((e) => ((error = errorText(e)), (phase = "error")));

  async function apply() {
    phase = "applying";
    try {
      data = await call(false);
      phase = "done";
      toast.success("Recovery complete");
      app.refresh();
      setTimeout(() => (open = false), 3000);
    } catch (e) {
      error = `Recovery failed: ${errorText(e)}`;
      phase = "error";
      toast.error("Recovery failed");
    }
  }

  function reason(j: FailedJob): { text: string; tone?: string; details: string } {
    if (j.likely_oom) {
      const peak =
        j.peak_memory_formatted && j.peak_memory_formatted !== "0.0 MB"
          ? ` (peak: ${j.peak_memory_formatted})`
          : "";
      return { text: "OOM", tone: "danger", details: (j.oom_reason ?? "") + peak };
    }
    if (j.likely_timeout) return { text: "Timeout", tone: "warning", details: j.timeout_reason ?? "" };
    return { text: "Unknown", details: "" };
  }
</script>

<Modal bind:open title="Recover workflow {workflowId}" size="lg" {onclose}>
  {#if phase === "loading" || phase === "applying"}
    <div class="empty">
      <span class="spinner"></span>
      {phase === "loading" ? "Analyzing workflow for recovery…" : "Applying recovery…"}
    </div>
  {:else if phase === "error"}
    <p class="error-text">{error}</p>
  {:else if phase === "done" && r}
    <section class="done">
      <h4>✓ Recovery complete</h4>
      <ul>
        {#if r.oom_fixed}<li>{r.oom_fixed} job(s) had memory increased</li>{/if}
        {#if r.timeout_fixed}<li>{r.timeout_fixed} job(s) had runtime increased</li>{/if}
        {#if r.unknown_retried}<li>{r.unknown_retried} job(s) with unknown failures reset</li>{/if}
        {#if retryCount}
          <li>Reset {retryCount} job(s)</li>
          <li>Slurm schedulers regenerated and submitted</li>
        {/if}
      </ul>
    </section>
  {:else if r}
    <section>
      <h4>Summary</h4>
      <Stats
        items={[
          { label: "Jobs to retry", value: retryCount, tone: retryCount ? "warning" : undefined },
          { label: "OOM fixes", value: r.oom_fixed ?? 0, tone: r.oom_fixed ? "danger" : undefined },
          {
            label: "Timeout fixes",
            value: r.timeout_fixed ?? 0,
            tone: r.timeout_fixed ? "warning" : undefined,
          },
          { label: "Unknown failures", value: r.other_failures ?? 0 },
        ]}
      />
    </section>

    {#if data?.diagnosis?.failed_jobs?.length}
      <section>
        <h4>Failed jobs</h4>
        <table>
          <thead><tr><th>Job</th><th>Return code</th><th>Reason</th><th>Details</th></tr></thead>
          <tbody>
            {#each data.diagnosis.failed_jobs as j, i (i)}
              {@const why = reason(j)}
              <tr>
                <td><code>{j.job_name}</code></td>
                <td>{j.return_code}</td>
                <td class={why.tone}>{why.text}</td>
                <td>{why.details}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </section>
    {/if}

    {#if r.adjustments?.length}
      <section>
        <h4>Resource adjustments</h4>
        <table>
          <thead><tr><th>Jobs</th><th>Memory</th><th>Runtime</th></tr></thead>
          <tbody>
            {#each r.adjustments as a, i (i)}
              <tr>
                <td>
                  {a.job_names.slice(0, 3).join(", ")}
                  {#if a.job_names.length > 3}<em class="muted">(+{a.job_names.length - 3} more)</em>{/if}
                </td>
                <td>
                  {#if a.memory_adjusted}<code>{a.original_memory}</code> →
                    <code>{a.new_memory}</code>{:else}—{/if}
                </td>
                <td>
                  {#if a.runtime_adjusted}<code>{a.original_runtime}</code> →
                    <code>{a.new_runtime}</code>{:else}—{/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </section>
    {/if}

    {#if r.slurm_dry_run?.planned_schedulers?.length}
      <section>
        <h4>Slurm allocations to create</h4>
        {#each r.slurm_dry_run.planned_schedulers as s (s.name)}
          <div class="sched">
            <strong>{s.name}</strong>
            <span class="muted">({s.job_count} job(s), {s.num_allocations} allocation(s))</span>
            <dl class="kv">
              <dt>Account</dt><dd>{s.account || "—"}</dd>
              <dt>Partition</dt><dd>{s.partition || "default"}</dd>
              <dt>Walltime</dt><dd>{s.walltime || "—"}</dd>
              <dt>Memory</dt><dd>{s.mem || "—"}</dd>
              <dt>Nodes</dt><dd>{s.nodes || 1}</dd>
            </dl>
          </div>
        {/each}
        <p class="muted">
          Total: {r.slurm_dry_run.total_allocations} allocation(s) would be submitted
        </p>
      </section>
    {/if}

    {#if !retryCount}
      <p class="note">
        {#if r.other_failures}
          {r.other_failures} job(s) failed with unknown causes. Use the CLI with
          <code>--retry-unknown</code> to retry these jobs.
        {:else}
          No recoverable jobs found. The workflow may have completed successfully or all failures
          require manual intervention.
        {/if}
      </p>
    {/if}
  {/if}

  {#snippet footer()}
    <button class="btn" onclick={() => (open = false)}>
      {phase === "preview" ? "Cancel" : "Close"}
    </button>
    {#if phase === "preview" && retryCount}
      <button class="btn btn-primary" onclick={apply}>Apply recovery</button>
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
  .danger {
    color: var(--danger);
  }
  .warning {
    color: var(--warning);
  }
  .sched {
    margin-bottom: 12px;
  }
  .sched .kv {
    margin-top: 6px;
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
