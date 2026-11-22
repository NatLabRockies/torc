<!-- `torc recover`: dry-run preview with adjustable options, then apply. -->
<script lang="ts">
  import { api, unwrap } from "../../lib/api/client";
  import { dashPost } from "../../lib/api/dash";
  import { errorText } from "../../lib/format";
  import { app } from "../../lib/state.svelte";
  import Modal from "../../lib/ui/Modal.svelte";
  import { toast } from "../../lib/ui/toast.svelte";
  import { loadOutputDir, resolveLogBaseDir } from "../debugging/logs";
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

  // Options, applied by re-running the preview. The output dir is where the jobs' logs are,
  // relative to the workflow's submission directory (as in the Debugging tab).
  let memoryMultiplier = $state(1.5);
  let runtimeMultiplier = $state(1.5);
  let retryUnknown = $state(false);
  let outputDir = $state(loadOutputDir());
  let submissionDir = $state<string | null>(null);
  /** Options as of the shown preview; Apply uses these so it does what the preview showed. */
  let previewed = $state<Record<string, unknown> | null>(null);

  function options() {
    return {
      memory_multiplier: memoryMultiplier,
      runtime_multiplier: runtimeMultiplier,
      retry_unknown: retryUnknown,
      output_dir: resolveLogBaseDir(outputDir.trim() || "torc_output", submissionDir),
    };
  }

  async function call(dry_run: boolean, opts: Record<string, unknown>): Promise<RecoverData> {
    const res = await dashPost<Response>("/api/cli/recover", { ...wid(workflowId), ...opts, dry_run });
    if (!res.success) throw new Error(res.error || "Recovery check failed");
    if (!res.data?.result) throw new Error("No recovery data available.");
    return res.data;
  }

  async function preview() {
    phase = "loading";
    const opts = options();
    try {
      data = await call(true, opts);
      previewed = opts;
      phase = "preview";
    } catch (e) {
      error = errorText(e);
      phase = "error";
    }
  }

  async function start() {
    try {
      const wf = await unwrap(api.GET("/workflows/{id}", { params: { path: { id: workflowId } } }));
      submissionDir = wf.submission_directory ?? null;
    } catch {
      // Fall back to torc-dash's working directory for a relative output dir.
    }
    preview();
  }
  start();

  async function apply() {
    phase = "applying";
    try {
      data = await call(false, previewed ?? options());
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
  {#if phase !== "applying" && phase !== "done"}
    <form
      class="options"
      onsubmit={(e) => {
        e.preventDefault();
        preview();
      }}
    >
      <label class="field">
        Memory multiplier
        <input class="input" type="number" min="1" step="0.1" bind:value={memoryMultiplier} />
        <span class="hint">For jobs that ran out of memory</span>
      </label>
      <label class="field">
        Runtime multiplier
        <input class="input" type="number" min="1" step="0.1" bind:value={runtimeMultiplier} />
        <span class="hint">For jobs that timed out</span>
      </label>
      <label class="field">
        Output dir
        <input class="input mono" bind:value={outputDir} placeholder="torc_output" spellcheck="false" />
        <span class="hint">Relative to the submission directory</span>
      </label>
      <label class="check">
        <input type="checkbox" bind:checked={retryUnknown} /> Also retry jobs that failed for unknown
        reasons
      </label>
      <button class="btn btn-sm" type="submit" disabled={phase === "loading"}>Update preview</button>
    </form>
  {/if}
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
          {r.other_failures} job(s) failed with unknown causes. Check “Also retry jobs that failed
          for unknown reasons” and update the preview to retry them.
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
  .options {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px 14px;
    align-items: end;
    margin-bottom: 18px;
    padding-bottom: 16px;
    border-bottom: 1px solid var(--border);
  }
  .options .check {
    grid-column: 1 / 3;
  }
  .options button {
    justify-self: end;
  }
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
