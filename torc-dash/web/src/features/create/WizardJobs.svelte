<script lang="ts">
  import { tick } from "svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import { toast } from "../../lib/ui/toast.svelte";
  import Chips from "./Chips.svelte";
  import Field from "./Field.svelte";
  import ItemCard from "./ItemCard.svelte";
  import {
    PRESETS,
    expandedCount,
    jobLabel,
    jobWarning,
    newJob,
    newScheduler,
    type PresetKey,
    type WizardData,
    type WizardJob,
  } from "./spec";

  let { w = $bindable(), err }: { w: WizardData; err: (key: string) => string | undefined } = $props();

  let open = $state<Record<number, boolean>>({});
  let list = $state<HTMLElement>();

  export async function add() {
    const job = newJob(w.nextId++);
    w.jobs.push(job);
    open[job.id] = true;
    await tick();
    list?.querySelector<HTMLInputElement>(`[data-id="${job.id}"] input`)?.focus();
  }

  function remove(id: number) {
    w.jobs = w.jobs.filter((j) => j.id !== id);
    for (const j of w.jobs) j.depends_on = j.depends_on.filter((d) => d !== id);
    for (const a of w.actions) a.jobs = a.jobs.filter((d) => d !== id);
  }

  function setPreset(job: WizardJob, key: PresetKey) {
    job.resource_preset = key;
    if (key !== "custom") {
      const p = PRESETS[key];
      job.num_cpus = p.num_cpus;
      job.memory = p.memory;
      job.num_gpus = p.num_gpus;
    }
  }

  function onSchedulerChange(job: WizardJob, value: string) {
    if (value !== "new") {
      job.scheduler = value ? Number(value) : null;
      return;
    }
    const s = newScheduler(w.nextId++);
    s.name = `scheduler-${w.schedulers.length + 1}`;
    w.schedulers.push(s);
    job.scheduler = s.id;
    toast.info(`Scheduler "${s.name}" created. Configure it in the Schedulers step.`);
  }

  function summary(j: WizardJob): string {
    const parts: string[] = [];
    if (j.command.trim()) parts.push(j.command.trim());
    const n = j.parameters.trim() ? expandedCount(j) : 1;
    if (n !== 1) parts.unshift(n === null ? "parameterized" : `×${n}`);
    return parts.join(" · ");
  }

  const jobHasError = (j: WizardJob) =>
    ["name", "command", "parameters", "num_cpus", "num_gpus", "memory", "runtime"].some((f) =>
      err(`job:${j.id}:${f}`),
    );
</script>

<div class="head">
  <div>
    <h4>Jobs</h4>
    <p class="muted">
      Need file-based dependencies or complex pipelines? Use a spec file instead (<a
        href="https://natlabrockies.github.io/torc/latest/core/workflows/creating-workflows.html"
        target="_blank"
        rel="noreferrer">guide</a
      >).
    </p>
  </div>
  <button type="button" class="btn btn-sm" onclick={add}><Icon name="plus" size={14} /> Add job</button>
</div>

{#if err("step")}<p class="error-text">{err("step")}</p>{/if}

<div class="list" bind:this={list}>
  {#each w.jobs as job, i (job.id)}
    {@const others = w.jobs.filter((j) => j.id !== job.id)}
    {@const warning = jobWarning(job)}
    {@const count = job.parameters.trim() ? expandedCount(job) : null}
    <div data-id={job.id}>
      <ItemCard
        index={i + 1}
        title={jobLabel(job)}
        subtitle={summary(job)}
        bind:open={open[job.id]}
        invalid={jobHasError(job)}
        onremove={() => remove(job.id)}
      >
        <Field label="Name" required error={err(`job:${job.id}:name`)} hint={warning ?? undefined}>
          <input class="input" bind:value={job.name} placeholder="e.g. process-data" />
        </Field>
        <Field label="Scheduler" hint="Auto lets torc pick a matching scheduler">
          <select
            class="select"
            value={job.scheduler ?? ""}
            onchange={(e) => onSchedulerChange(job, e.currentTarget.value)}
          >
            <option value="">Auto</option>
            {#each w.schedulers.filter((s) => s.name.trim()) as s (s.id)}
              <option value={s.id}>{s.name}</option>
            {/each}
            <option value="new">+ New Slurm scheduler…</option>
          </select>
        </Field>
        <Field label="Command" required wide error={err(`job:${job.id}:command`)}>
          <input
            class="input mono"
            bind:value={job.command}
            placeholder="e.g. python process.py --input data.csv"
          />
        </Field>
        <Chips
          label="Depends on"
          options={others.map((o) => ({ id: o.id, label: jobLabel(o) }))}
          bind:selected={job.depends_on}
          empty="Add more jobs to define dependencies."
        />
        <Field
          label="Parameters"
          error={err(`job:${job.id}:parameters`)}
          hint={count === null
            ? "Expands into multiple jobs. Use {param} in the name and command."
            : `Expands into ${count} job${count === 1 ? "" : "s"}.`}
        >
          <input
            class="input mono"
            bind:value={job.parameters}
            placeholder="i: 1:10   or   x: [1,2,3], y: [a,b,c]"
          />
        </Field>
        <Field label="Parameter mode" hint="Product: every combination. Zip: pair values by position.">
          <select class="select" bind:value={job.parameter_mode} disabled={!job.parameters.trim()}>
            <option value="product">Product</option>
            <option value="zip">Zip</option>
          </select>
        </Field>

        {#if w.strategy === "resource_aware"}
          <div class="field wide" role="group" aria-label="Resources">
            <span>Resources</span>
            <div class="presets">
              {#each Object.entries(PRESETS) as [key, p] (key)}
                <button
                  type="button"
                  class="preset"
                  aria-pressed={job.resource_preset === key}
                  onclick={() => setPreset(job, key as PresetKey)}
                >
                  <strong>{p.label}</strong>
                  {#if key !== "custom"}
                    <small>{p.num_cpus} CPU · {p.memory}{p.num_gpus ? ` · ${p.num_gpus} GPU` : ""}</small>
                  {:else}
                    <small>Set your own</small>
                  {/if}
                </button>
              {/each}
            </div>
          </div>
          {#if job.resource_preset === "custom"}
            <div class="res wide">
              <Field label="CPUs" error={err(`job:${job.id}:num_cpus`)}>
                <input class="input" type="number" min="1" bind:value={job.num_cpus} />
              </Field>
              <Field label="Memory" error={err(`job:${job.id}:memory`)}>
                <input class="input" bind:value={job.memory} placeholder="4g" />
              </Field>
              <Field label="GPUs" error={err(`job:${job.id}:num_gpus`)}>
                <input class="input" type="number" min="0" bind:value={job.num_gpus} />
              </Field>
              <Field label="Runtime" error={err(`job:${job.id}:runtime`)} hint="ISO8601, e.g. PT1H">
                <input class="input" bind:value={job.runtime} placeholder="PT1H" />
              </Field>
            </div>
          {:else}
            <Field label="Runtime" error={err(`job:${job.id}:runtime`)} hint="ISO8601 duration, e.g. PT30M, PT2H">
              <input class="input" bind:value={job.runtime} placeholder="PT1H" />
            </Field>
          {/if}
        {/if}
      </ItemCard>
    </div>
  {:else}
    <button type="button" class="empty-add" onclick={add}>
      <Icon name="plus" size={20} />
      <span>Add your first job</span>
      <small class="faint">A job is a command plus its dependencies and resources.</small>
    </button>
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
  .mono {
    font-family: var(--mono);
    font-size: 12.5px;
  }
  .wide {
    grid-column: 1 / -1;
  }
  .presets {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(110px, 1fr));
    gap: 6px;
  }
  .preset {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
    padding: 7px 10px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--surface);
    color: var(--text);
    font: inherit;
    cursor: pointer;
    text-align: left;
  }
  .preset small {
    color: var(--text-muted);
    font-size: 11.5px;
    font-weight: 400;
  }
  .preset:hover {
    background: var(--surface-hover);
  }
  .preset[aria-pressed="true"] {
    border-color: var(--accent);
    background: var(--accent-soft);
    box-shadow: inset 0 0 0 1px var(--accent);
  }
  .preset:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .res {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 12px;
  }
  .empty-add {
    display: grid;
    place-items: center;
    gap: 4px;
    padding: 32px 16px;
    border: 1.5px dashed var(--border-strong);
    border-radius: var(--radius-lg);
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    cursor: pointer;
  }
  .empty-add:hover {
    border-color: var(--accent);
    color: var(--accent);
  }
</style>
