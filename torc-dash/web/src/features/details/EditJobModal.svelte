<!--
  Edit one job's priority and resource requirements. A requirement shared with other jobs is
  cloned for this job (see setJobResources), so the others are not affected.
-->
<script lang="ts">
  import { api, unwrap, type Job, type ResourceRequirements } from "../../lib/api/client";
  import { errorText } from "../../lib/format";
  import { app } from "../../lib/state.svelte";
  import Modal from "../../lib/ui/Modal.svelte";
  import { toast } from "../../lib/ui/toast.svelte";
  import { JOB_BUSY, setJobResources } from "./editing";
  import ResourceInputs from "./ResourceInputs.svelte";

  let {
    job,
    resources,
    onclose,
  }: { job: Job; resources: ResourceRequirements | null; onclose: () => void } = $props();

  let open = $state(true);
  // Copies of the props: this form edits them locally until Save.
  let priority = $state(initial().priority);
  let fields = $state(initial().fields);
  let saving = $state(false);
  let error = $state("");

  function initial() {
    return {
      priority: job.priority ?? 0,
      fields: {
        num_cpus: resources?.num_cpus ?? 1,
        memory: resources?.memory ?? "1m",
        num_gpus: resources?.num_gpus ?? 0,
        num_nodes: resources?.num_nodes ?? 1,
        runtime: resources?.runtime ?? "PT1H",
      },
    };
  }

  let busy = $derived(JOB_BUSY.has(job.status ?? ""));
  let resourcesChanged = $derived(
    !resources ||
      (Object.keys(fields) as (keyof typeof fields)[]).some((k) => fields[k] !== resources[k]),
  );

  async function save(e: SubmitEvent) {
    e.preventDefault();
    saving = true;
    error = "";
    try {
      let note = "";
      if (resourcesChanged) {
        const how = await setJobResources(job, resources, fields);
        if (how === "cloned") note = " (gave it its own resource requirements)";
      }
      if (priority !== (job.priority ?? 0)) {
        // Re-read the job: a clone above changed its resource_requirements_id.
        const fresh = await unwrap(api.GET("/jobs/{id}", { params: { path: { id: job.id! } } }));
        await unwrap(
          api.PUT("/jobs/{id}", { params: { path: { id: job.id! } }, body: { ...fresh, priority } }),
        );
      }
      toast.success(`Job ${job.name} updated${note}`);
      app.refresh();
      open = false;
    } catch (err) {
      error = errorText(err);
    } finally {
      saving = false;
    }
  }
</script>

<Modal bind:open title="Edit job {job.name}" size="md" {onclose}>
  <form id="edit-job" class="form" onsubmit={save}>
    {#if busy}
      <p class="note">This job is {job.status}. Edit it after it finishes.</p>
    {/if}
    <label class="field">
      Priority
      <input class="input" type="number" min="0" step="1" bind:value={priority} disabled={busy} required />
      <span class="hint">Ready jobs with higher priority are started first.</span>
    </label>
    <h4>Resources</h4>
    <ResourceInputs bind:fields disabled={busy} />
    {#if resources && resourcesChanged}
      <p class="muted">
        If <code>{resources.name}</code> is shared with other jobs, this job gets its own copy and
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
