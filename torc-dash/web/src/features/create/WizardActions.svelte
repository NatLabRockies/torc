<script lang="ts">
  import Icon from "../../lib/ui/Icon.svelte";
  import Chips from "./Chips.svelte";
  import Field from "./Field.svelte";
  import ItemCard from "./ItemCard.svelte";
  import {
    TRIGGERS,
    isJobTriggered,
    jobLabel,
    newAction,
    type WizardAction,
    type WizardData,
  } from "./spec";

  let {
    w = $bindable(),
    err,
    slurmAuto,
  }: { w: WizardData; err: (key: string) => string | undefined; slurmAuto: boolean } = $props();

  let open = $state<Record<number, boolean>>({});

  const schedulers = $derived(w.schedulers.filter((s) => s.name.trim()));

  export function add() {
    const a = newAction(w.nextId++);
    if (schedulers.length === 1) a.scheduler = schedulers[0].id;
    w.actions.push(a);
    open[a.id] = true;
  }

  function remove(id: number) {
    w.actions = w.actions.filter((a) => a.id !== id);
  }

  const triggerLabel = (a: WizardAction) => TRIGGERS.find((t) => t.id === a.trigger_type)?.label ?? "";
  const hasError = (a: WizardAction) =>
    ["scheduler", "jobs", "num_allocations"].some((f) => err(`action:${a.id}:${f}`));
</script>

<div class="head">
  <div>
    <h4>Actions</h4>
    <p class="muted">
      Optional. Schedule Slurm nodes automatically when the workflow starts, or when jobs become
      ready or complete.
    </p>
  </div>
  <button type="button" class="btn btn-sm" onclick={add} disabled={schedulers.length === 0}>
    <Icon name="plus" size={14} /> Add action
  </button>
</div>

<div class="list">
  {#if schedulers.length === 0 && !slurmAuto}
    <div class="note">
      <strong>No schedulers available.</strong>
      <span>Define a scheduler in the previous step, or enable "Generate Slurm schedulers" under Options.</span>
    </div>
  {:else if w.actions.length === 0}
    <div class="note">
      {#if slurmAuto}
        <strong>Actions will be generated automatically.</strong>
        <span>"Generate Slurm schedulers" is on, so actions are created to schedule nodes when jobs are ready.</span>
      {:else}
        <strong>No actions defined.</strong>
        <span>Actions are optional for local execution.</span>
      {/if}
    </div>
  {/if}

  {#each w.actions as a, i (a.id)}
    <ItemCard
      index={i + 1}
      title={triggerLabel(a)}
      subtitle={w.schedulers.find((s) => s.id === a.scheduler)?.name}
      bind:open={open[a.id]}
      invalid={hasError(a)}
      onremove={() => remove(a.id)}
    >
      <Field label="Trigger" required>
        <select
          class="select"
          bind:value={a.trigger_type}
          onchange={() => {
            if (!isJobTriggered(a.trigger_type)) a.jobs = [];
          }}
        >
          {#each TRIGGERS as t (t.id)}<option value={t.id}>{t.label}</option>{/each}
        </select>
      </Field>
      <Field label="Scheduler" required error={err(`action:${a.id}:scheduler`)}>
        <select class="select" bind:value={a.scheduler}>
          <option value={null}>Select scheduler…</option>
          {#each schedulers as s (s.id)}<option value={s.id}>{s.name}</option>{/each}
        </select>
      </Field>
      {#if isJobTriggered(a.trigger_type)}
        <Chips
          label="Jobs *"
          options={w.jobs.map((j) => ({ id: j.id, label: jobLabel(j) }))}
          bind:selected={a.jobs}
          empty="No jobs defined."
          error={err(`action:${a.id}:jobs`)}
        />
      {/if}
      <Field label="Number of allocations" error={err(`action:${a.id}:num_allocations`)}>
        <input class="input" type="number" min="1" bind:value={a.num_allocations} />
      </Field>
      {#if w.strategy === "queue_depth"}
        <Field label="Max parallel jobs">
          <input class="input" type="number" min="1" bind:value={a.max_parallel_jobs} />
        </Field>
      {:else}
        <span></span>
      {/if}
      <label class="check">
        <input type="checkbox" bind:checked={a.start_one_worker_per_node} />
        Start one worker per node
      </label>
    </ItemCard>
  {/each}
</div>

<style>
  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 12px;
  }
  .head p {
    margin: 2px 0 0;
    font-size: 12.5px;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .note {
    display: grid;
    gap: 4px;
    padding: 20px;
    border: 1.5px dashed var(--border-strong);
    border-radius: var(--radius-lg);
    color: var(--text-muted);
    text-align: center;
  }
  .note strong {
    color: var(--text);
  }
</style>
