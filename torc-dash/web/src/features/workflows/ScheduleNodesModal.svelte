<!-- `torc slurm schedule-nodes`: submit more Slurm allocations for one of the workflow's schedulers. -->
<script lang="ts">
  import { api, listAll, unwrap, type SlurmScheduler } from "../../lib/api/client";
  import { op } from "../../lib/api/dash";
  import { errorText } from "../../lib/format";
  import { app } from "../../lib/state.svelte";
  import Modal from "../../lib/ui/Modal.svelte";
  import { toast } from "../../lib/ui/toast.svelte";
  import { loadOutputDir, resolveLogBaseDir } from "../debugging/logs";

  let { workflowId, onclose }: { workflowId: number; onclose: () => void } = $props();

  let open = $state(true);
  let schedulers = $state<SlurmScheduler[] | null>(null);
  let submissionDir = $state<string | null>(null);
  let loadError = $state("");
  let schedulerId = $state<number | null>(null);
  let numAllocations = $state(1);
  let maxParallelJobs = $state<number | null>(null);
  let suppressActions = $state(true);
  let oneWorkerPerNode = $state(false);
  let outputDir = $state(loadOutputDir());
  let busy = $state(false);
  let error = $state("");

  async function load() {
    const id = workflowId;
    try {
      const [s, wf] = await Promise.all([
        listAll((offset, limit) =>
          unwrap(api.GET("/slurm_schedulers", { params: { query: { workflow_id: id, offset, limit } } })),
        ),
        unwrap(api.GET("/workflows/{id}", { params: { path: { id } } })),
      ]);
      schedulers = s;
      schedulerId = s[0]?.id ?? null;
      submissionDir = wf.submission_directory ?? null;
    } catch (e) {
      loadError = errorText(e);
    }
  }
  load();

  let chosen = $derived(schedulers?.find((s) => s.id === schedulerId));

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (schedulerId === null) return;
    busy = true;
    error = "";
    try {
      await op("schedule-nodes", {
        workflow_id: workflowId,
        scheduler_config_id: schedulerId,
        num_allocations: numAllocations,
        max_parallel_jobs: maxParallelJobs || undefined,
        suppress_actions: suppressActions,
        start_one_worker_per_node: oneWorkerPerNode,
        output_dir: resolveLogBaseDir(outputDir.trim() || "torc_output", submissionDir),
        cwd: submissionDir ?? undefined,
      });
      toast.success(`Submitted ${numAllocations} Slurm allocation(s) for workflow ${workflowId}`);
      app.refresh();
      open = false;
    } catch (err) {
      error = errorText(err);
    } finally {
      busy = false;
    }
  }
</script>

<Modal bind:open title="Schedule Slurm nodes for workflow {workflowId}" size="md" {onclose}>
  {#if loadError}
    <p class="error-text">{loadError}</p>
  {:else if !schedulers}
    <div class="empty"><span class="spinner"></span> Loading schedulers…</div>
  {:else if schedulers.length === 0}
    <p class="muted">
      This workflow has no Slurm schedulers. Add <code>slurm_schedulers</code> to the spec, or create
      the workflow with “Generate Slurm schedulers”.
    </p>
  {:else}
    <form id="schedule-form" class="form" onsubmit={submit}>
      <p class="muted">
        Submit Slurm allocations that run this workflow's ready jobs. Use this to add capacity or to
        run jobs again after resetting them. Uninitialized workflows are initialized first.
      </p>
      <label class="field">
        Scheduler
        <select class="input" bind:value={schedulerId}>
          {#each schedulers as s (s.id)}
            <option value={s.id}>{s.name || `Scheduler ${s.id}`}</option>
          {/each}
        </select>
        {#if chosen}
          <span class="hint mono">
            account={chosen.account} nodes={chosen.nodes} walltime={chosen.walltime}
            {chosen.partition ? `partition=${chosen.partition}` : ""}
            {chosen.mem ? `mem=${chosen.mem}` : ""}
          </span>
        {/if}
      </label>
      <div class="row">
        <label class="field">
          Allocations
          <input class="input" type="number" min="1" bind:value={numAllocations} required />
        </label>
        <label class="field">
          Max parallel jobs
          <input class="input" type="number" min="1" bind:value={maxParallelJobs} placeholder="Unlimited" />
        </label>
      </div>
      <label class="field">
        Output dir
        <input class="input mono" bind:value={outputDir} placeholder="torc_output" spellcheck="false" />
        <span class="hint">Relative to the submission directory</span>
      </label>
      <label class="check">
        <input type="checkbox" bind:checked={suppressActions} /> Skip the workflow's pending
        schedule_nodes actions
      </label>
      <span class="hint indent">
        Otherwise the new workers may also fire actions re-armed by a reset and submit extra
        allocations.
      </span>
      <label class="check">
        <input type="checkbox" bind:checked={oneWorkerPerNode} /> Start one worker per node
      </label>
      {#if error}<p class="error-text">{error}</p>{/if}
    </form>
  {/if}
  {#snippet footer()}
    <button class="btn" onclick={() => (open = false)}>Cancel</button>
    {#if schedulers?.length}
      <button class="btn btn-primary" type="submit" form="schedule-form" disabled={busy}>
        {busy ? "Submitting…" : "Submit allocations"}
      </button>
    {/if}
  {/snippet}
</Modal>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
  }
  .indent {
    margin-top: -10px;
    padding-left: 24px;
    color: var(--text-muted);
    font-size: 12px;
  }
  p {
    margin: 0;
  }
</style>
