<!--
  Edit one job's command, priority, and resource requirements. The job can use another existing
  requirement or new values; a requirement shared with other jobs is cloned for this job (see
  setJobResources), so the others are not affected. Command changes follow commandEditMode.
-->
<script lang="ts">
  import { api, listAll, unwrap, type Job, type ResourceRequirements } from "../../lib/api/client";
  import { cli } from "../../lib/api/dash";
  import { errorText } from "../../lib/format";
  import { app } from "../../lib/state.svelte";
  import Modal from "../../lib/ui/Modal.svelte";
  import { toast } from "../../lib/ui/toast.svelte";
  import { JOB_BUSY, commandEditMode, setJobResources, type ResourceFields } from "./editing";
  import ResourceInputs from "./ResourceInputs.svelte";

  let {
    job,
    resources,
    onclose,
  }: { job: Job; resources: ResourceRequirements | null; onclose: () => void } = $props();

  let open = $state(true);
  // Copies of the props: this form edits them locally until Save.
  let command = $state(initial().command);
  let priority = $state(initial().priority);
  let rrId = $state(initial().rrId);
  let fields = $state(initial().fields);
  let records = $state<ResourceRequirements[]>(initial().records);
  let mode = $state<"direct" | "reset" | string | undefined>(undefined);
  let saving = $state(false);
  let error = $state("");

  function initial() {
    return {
      command: job.command,
      priority: job.priority ?? 0,
      rrId: resources?.id ?? null,
      fields: valuesOf(resources),
      records: resources ? [resources] : [],
    };
  }

  function valuesOf(r: ResourceRequirements | null | undefined): ResourceFields {
    return {
      num_cpus: r?.num_cpus ?? 1,
      memory: r?.memory ?? "1m",
      num_gpus: r?.num_gpus ?? 0,
      num_nodes: r?.num_nodes ?? 1,
      runtime: r?.runtime ?? "PT1H",
    };
  }

  function load() {
    listAll((offset, limit) =>
      unwrap(
        api.GET("/resource_requirements", {
          params: { query: { workflow_id: job.workflow_id, offset, limit } },
        }),
      ),
    )
      .then((r) => (records = r))
      .catch(() => {});
    commandEditMode(job)
      .then((m) => (mode = m))
      .catch((e) => (mode = `Could not check job status: ${errorText(e)}`));
  }
  load();

  let busy = $derived(JOB_BUSY.has(job.status ?? ""));
  let selected = $derived(records.find((r) => r.id === rrId) ?? null);
  let commandEditable = $derived(mode === "direct" || mode === "reset");
  let commandChanged = $derived(command.trim() !== job.command);
  let rrChanged = $derived(rrId !== (resources?.id ?? null));
  let fieldsChanged = $derived(
    !selected ||
      (Object.keys(fields) as (keyof ResourceFields)[]).some((k) => fields[k] !== selected[k]),
  );

  /** Picking another requirement shows its values, which can then be adjusted. */
  function pick(id: number) {
    rrId = id;
    fields = valuesOf(records.find((r) => r.id === id));
  }

  const getJob = () => unwrap(api.GET("/jobs/{id}", { params: { path: { id: job.id! } } }));
  const putJob = (body: Job) =>
    unwrap(api.PUT("/jobs/{id}", { params: { path: { id: job.id! } }, body }));

  async function save(e: SubmitEvent) {
    e.preventDefault();
    if (commandChanged && !command.trim()) return (error = "The command can't be empty.");
    saving = true;
    error = "";
    const wid = { workflow_id: String(job.workflow_id) };
    // Mode may be stale: re-check right before a reset so nothing that started since is lost.
    const reset = commandChanged && (await commandEditMode(job).catch(() => "")) === "reset";
    if (commandChanged && !reset && job.status !== "uninitialized") {
      saving = false;
      return (error = "The command can no longer be changed; reopen this dialog to see why.");
    }
    let note = "";
    try {
      if (reset) await cli("reset-status", wid);
      if (rrChanged && rrId !== null) {
        await putJob({ ...(await getJob()), resource_requirements_id: rrId });
      }
      if (fieldsChanged) {
        const how = await setJobResources(await getJob(), selected, fields);
        if (how === "cloned") note = " (gave it its own resource requirements)";
      }
      if (commandChanged || priority !== (job.priority ?? 0)) {
        // Re-read the job: the steps above may have changed its status or requirement.
        await putJob({ ...(await getJob()), command: command.trim(), priority });
      }
      toast.success(`Job ${job.name} updated${note}`);
      open = false;
    } catch (err) {
      error = errorText(err);
    } finally {
      // Restore the workflow's state even if an edit failed after the reset. It was initialized
      // before, so skip the missing-input-files check that initialization already passed.
      if (reset) {
        await cli("initialize", { ...wid, force: true }).catch((err) =>
          toast.error(`Workflow ${job.workflow_id} was reset but not initialized: ${errorText(err)}`),
        );
      }
      saving = false;
      app.refresh();
    }
  }
</script>

<Modal bind:open title="Edit job {job.name}" size="md" {onclose}>
  <form id="edit-job" class="form" onsubmit={save}>
    {#if busy}
      <p class="note">This job is {job.status}. Edit it after it finishes.</p>
    {/if}
    <label class="field">
      Command
      <textarea
        class="input mono"
        rows="3"
        bind:value={command}
        disabled={busy || !commandEditable}
        spellcheck="false"
      ></textarea>
      {#if mode === undefined}
        <span class="hint">Checking whether the command can change…</span>
      {:else if mode === "reset"}
        <span class="hint">
          Changing it resets the workflow's jobs and initializes it again. Nothing has run yet, so
          nothing is lost.
        </span>
      {:else if mode !== "direct"}
        <span class="hint">{mode}</span>
      {/if}
    </label>
    <label class="field">
      Priority
      <input class="input" type="number" min="0" step="1" bind:value={priority} disabled={busy} required />
      <span class="hint">Ready jobs with higher priority are started first.</span>
    </label>
    <h4>Resources</h4>
    <label class="field">
      Resource requirements
      <select
        class="select"
        value={rrId}
        onchange={(e) => pick(Number(e.currentTarget.value))}
        disabled={busy || records.length === 0}
      >
        {#each records as r (r.id)}
          <option value={r.id}>{r.name}</option>
        {/each}
      </select>
    </label>
    <ResourceInputs bind:fields disabled={busy} />
    {#if selected && fieldsChanged}
      <p class="muted">
        If <code>{selected.name}</code> is shared with other jobs, this job gets its own copy and
        the others keep their values.
      </p>
    {/if}
    <p class="muted">New values apply the next time the job runs. Reset it to rerun it.</p>
    {#if error}<p class="error-text">{error}</p>{/if}
  </form>
  {#snippet footer()}
    <button class="btn" onclick={() => (open = false)}>Cancel</button>
    <button class="btn btn-primary" type="submit" form="edit-job" disabled={saving || busy}>
      {saving ? "Saving…" : "Save"}
    </button>
  {/snippet}
</Modal>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  h4 {
    margin: 4px 0 0;
    font-size: 13px;
  }
  p {
    margin: 0;
  }
  .note {
    padding: 10px 12px;
    border-left: 3px solid var(--warning);
    border-radius: var(--radius);
    background: var(--surface-2);
  }
</style>
