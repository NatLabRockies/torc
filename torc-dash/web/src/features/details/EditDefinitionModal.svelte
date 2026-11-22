<!--
  Add a resource requirement or a Slurm scheduler (a record without an id), or edit one in place.
  Every job (or allocation) that uses an existing record picks up the change, so editing is
  allowed only while nothing in the workflow is running. Adding is always allowed.
-->
<script lang="ts">
  import { api, unwrap, type ResourceRequirements, type SlurmScheduler } from "../../lib/api/client";
  import { errorText } from "../../lib/format";
  import { app } from "../../lib/state.svelte";
  import Modal from "../../lib/ui/Modal.svelte";
  import { toast } from "../../lib/ui/toast.svelte";
  import { editBlockedReason, jobsUsing } from "./editing";
  import ResourceInputs from "./ResourceInputs.svelte";

  type Target =
    | { kind: "resources"; record: ResourceRequirements }
    | { kind: "scheduler"; record: SlurmScheduler };

  let { target, onclose }: { target: Target; onclose: () => void } = $props();

  let open = $state(true);
  // Local copies of the record, saved on submit.
  const copy = () => ({
    rr: target.kind === "resources" ? { ...target.record } : null,
    sched: target.kind === "scheduler" ? { ...target.record } : null,
  });
  let rr = $state(copy().rr);
  let sched = $state(copy().sched);
  let blocked = $state<string | null | undefined>(undefined);
  let users = $state<number | null>(null);
  let saving = $state(false);
  let error = $state("");

  const isNew = $derived(target.record.id == null);

  async function check() {
    if (isNew) return (blocked = null);
    const wf = target.record.workflow_id;
    blocked = await editBlockedReason(wf).catch((e) => `Could not check job status: ${errorText(e)}`);
    if (target.kind === "resources") {
      users = await jobsUsing(wf, target.record.id!).catch(() => null);
    }
  }
  check();

  async function save(e: SubmitEvent) {
    e.preventDefault();
    saving = true;
    error = "";
    try {
      // Work may have started since the dialog opened.
      // ponytail: check-then-update is not atomic; enforce on the server if that window matters.
      if (!isNew) {
        blocked = await editBlockedReason(target.record.workflow_id);
        if (blocked) return;
      }
      if (rr) {
        await unwrap(
          isNew
            ? api.POST("/resource_requirements", { body: rr })
            : api.PUT("/resource_requirements/{id}", { params: { path: { id: rr.id! } }, body: rr }),
        );
      } else if (sched) {
        // Optional text fields: on update the server keeps a null value and clears an empty one;
        // a new scheduler leaves blank ones unset.
        const body = { ...sched };
        for (const k of ["partition", "qos", "mem", "gres", "tmp", "extra"] as const) {
          body[k] = body[k]?.trim() || (isNew ? null : "");
        }
        await unwrap(
          isNew
            ? api.POST("/slurm_schedulers", { body })
            : api.PUT("/slurm_schedulers/{id}", { params: { path: { id: sched.id! } }, body }),
        );
      }
      toast.success(isNew ? "Added" : "Saved");
      app.refresh();
      open = false;
    } catch (err) {
      error = errorText(err);
    } finally {
      saving = false;
    }
  }

  let title = $derived(
    isNew
      ? target.kind === "resources"
        ? "Add resource requirements"
        : "Add Slurm scheduler"
      : target.kind === "resources"
        ? `Edit resource requirements ${target.record.name}`
        : `Edit Slurm scheduler ${target.record.name ?? target.record.id}`,
  );
  let disabled = $derived(blocked !== null);
</script>

<Modal bind:open {title} size="lg" {onclose}>
  <form id="edit-definition" class="form" onsubmit={save}>
    {#if blocked === undefined}
      <p class="muted"><span class="spinner small"></span> Checking for running jobs…</p>
    {:else if blocked}
      <p class="note">
        {blocked} Edit this after they finish, or cancel the workflow. To change one job's resources,
        edit the job instead.
      </p>
    {/if}

    {#if rr}
      <label class="field">
        Name
        <input class="input" bind:value={rr.name} {disabled} required />
      </label>
      <ResourceInputs bind:fields={rr} {disabled} />
      {#if users !== null}
        <p class="muted">
          Used by {users} job(s). All of them get the new values the next time they run.
        </p>
      {/if}
    {:else if sched}
      <div class="grid">
        <label class="field">
          Name
          <input class="input" bind:value={sched.name} {disabled} />
        </label>
        <label class="field">
          Account
          <input class="input" bind:value={sched.account} {disabled} required />
        </label>
        <label class="field">
          Partition
          <input class="input" bind:value={sched.partition} {disabled} placeholder="Default" />
        </label>
        <label class="field">
          QoS
          <input class="input" bind:value={sched.qos} {disabled} />
        </label>
        <label class="field">
          Nodes
          <input class="input" type="number" min="1" bind:value={sched.nodes} {disabled} required />
        </label>
        <label class="field">
          Walltime
          <input class="input mono" bind:value={sched.walltime} {disabled} required placeholder="04:00:00" />
        </label>
        <label class="field">
          Memory
          <input class="input mono" bind:value={sched.mem} {disabled} placeholder="e.g. 240G" />
        </label>
        <label class="field">
          GRES
          <input class="input mono" bind:value={sched.gres} {disabled} placeholder="e.g. gpu:2" />
        </label>
        <label class="field">
          Tmp
          <input class="input mono" bind:value={sched.tmp} {disabled} />
        </label>
      </div>
      <label class="field">
        Extra sbatch options
        <input class="input mono" bind:value={sched.extra} {disabled} placeholder="--constraint=…" />
      </label>
      {#if !isNew}<p class="muted">Applies to allocations submitted from now on.</p>{/if}
    {/if}
    {#if error}<p class="error-text">{error}</p>{/if}
  </form>
  {#snippet footer()}
    <button class="btn" onclick={() => (open = false)}>Cancel</button>
    <button class="btn btn-primary" type="submit" form="edit-definition" disabled={saving || disabled}>
      {saving ? "Saving…" : isNew ? "Add" : "Save"}
    </button>
  {/snippet}
</Modal>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(160px, 1fr));
    gap: 12px;
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
  .spinner.small {
    width: 12px;
    height: 12px;
  }
</style>
