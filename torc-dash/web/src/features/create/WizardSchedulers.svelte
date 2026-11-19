<script lang="ts">
  import { tick } from "svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import Field from "./Field.svelte";
  import ItemCard from "./ItemCard.svelte";
  import { newScheduler, type WizardData, type WizardScheduler } from "./spec";

  let {
    w = $bindable(),
    err,
    slurmAuto,
  }: { w: WizardData; err: (key: string) => string | undefined; slurmAuto: boolean } = $props();

  let open = $state<Record<number, boolean>>({});
  let list = $state<HTMLElement>();

  export async function add() {
    const s = newScheduler(w.nextId++);
    w.schedulers.push(s);
    open[s.id] = true;
    await tick();
    list?.querySelector<HTMLInputElement>(`[data-id="${s.id}"] input`)?.focus();
  }

  function remove(id: number) {
    w.schedulers = w.schedulers.filter((s) => s.id !== id);
    for (const j of w.jobs) if (j.scheduler === id) j.scheduler = null;
    for (const a of w.actions) if (a.scheduler === id) a.scheduler = null;
  }

  const fields = ["name", "account", "nodes", "walltime"];
  const hasError = (s: WizardScheduler) => fields.some((f) => err(`sched:${s.id}:${f}`));
  const usedBy = (s: WizardScheduler) => w.jobs.filter((j) => j.scheduler === s.id).length;
</script>

<div class="head">
  <div>
    <h4>Slurm schedulers</h4>
    <p class="muted">Optional. Configure Slurm allocations for running jobs on compute nodes.</p>
  </div>
  <button type="button" class="btn btn-sm" onclick={add}>
    <Icon name="plus" size={14} /> Add scheduler
  </button>
</div>

<div class="list" bind:this={list}>
  {#each w.schedulers as s, i (s.id)}
    {@const n = usedBy(s)}
    <div data-id={s.id}>
      <ItemCard
        index={i + 1}
        title={s.name.trim() || "Untitled scheduler"}
        subtitle={[s.account, `${s.nodes} node${s.nodes === 1 ? "" : "s"}`, s.walltime, n ? `${n} job${n === 1 ? "" : "s"}` : ""]
          .filter(Boolean)
          .join(" · ")}
        bind:open={open[s.id]}
        invalid={hasError(s)}
        onremove={() => remove(s.id)}
      >
        <Field label="Name" required error={err(`sched:${s.id}:name`)}>
          <input class="input" bind:value={s.name} placeholder="e.g. compute" />
        </Field>
        <Field label="Account" required error={err(`sched:${s.id}:account`)}>
          <input class="input" bind:value={s.account} placeholder="e.g. my_project" />
        </Field>
        <Field label="Nodes" error={err(`sched:${s.id}:nodes`)}>
          <input class="input" type="number" min="1" bind:value={s.nodes} />
        </Field>
        <Field label="Wall time" error={err(`sched:${s.id}:walltime`)} hint="HH:MM:SS">
          <input class="input" bind:value={s.walltime} placeholder="01:00:00" />
        </Field>
        <Field label="Partition">
          <input class="input" bind:value={s.partition} placeholder="e.g. compute" />
        </Field>
        <Field label="QoS">
          <input class="input" bind:value={s.qos} placeholder="e.g. normal" />
        </Field>
        <Field label="GRES">
          <input class="input" bind:value={s.gres} placeholder="e.g. gpu:2" />
        </Field>
        <Field label="Memory">
          <input class="input" bind:value={s.mem} placeholder="e.g. 256G" />
        </Field>
        <Field label="Temporary storage">
          <input class="input" bind:value={s.tmp} placeholder="e.g. 100G" />
        </Field>
        <Field label="Extra Slurm options">
          <input class="input" bind:value={s.extra} placeholder="e.g. --exclusive" />
        </Field>
      </ItemCard>
    </div>
  {:else}
    <div class="note">
      {#if slurmAuto}
        <strong>Schedulers will be generated automatically.</strong>
        <span>"Generate Slurm schedulers" is on, so schedulers are created from your jobs' resource requirements.</span>
      {:else}
        <strong>No schedulers defined.</strong>
        <span>
          Not needed for local runs. Add one to run on Slurm, or enable "Generate Slurm schedulers"
          under Options.
        </span>
      {/if}
    </div>
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
