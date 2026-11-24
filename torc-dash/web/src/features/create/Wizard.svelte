<!--
  Step-by-step workflow builder. State lives in the parent (bind:w) so it survives switching
  source tabs. The parent drives navigation through next()/back()/goTo() and creates the workflow
  when next() returns false on the last step.
-->
<script lang="ts">
  import { highlightJson } from "../../lib/highlight";
  import { tick } from "svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import { toast } from "../../lib/ui/toast.svelte";
  import { validateSpec, type ValidationReport } from "./actions";
  import ValidationPanel from "./ValidationPanel.svelte";
  import WizardActions from "./WizardActions.svelte";
  import WizardJobs from "./WizardJobs.svelte";
  import WizardSchedulers from "./WizardSchedulers.svelte";
  import {
    STEPS,
    expandedCount,
    firstInvalidStep,
    generateSpec,
    stepErrors,
    type WizardData,
  } from "./spec";

  let {
    w = $bindable(),
    slurm,
  }: {
    w: WizardData;
    /** Auto-generated Slurm schedulers (from the modal's options), or null when off. */
    slurm: { account: string; profile: string | null } | null;
  } = $props();

  const LAST = STEPS.length;

  /** Steps the user tried to leave; their "required" errors become visible. */
  let attempted = $state<Record<number, boolean>>({});
  let showPreview = $state(true);
  let body = $state<HTMLElement>();

  const errors = $derived(stepErrors(w, w.step));
  const specJson = $derived(JSON.stringify(generateSpec(w), null, 2));
  const totalJobs = $derived(
    w.jobs.reduce<number | null>((n, j) => {
      const c = expandedCount(j);
      return n === null || c === null ? null : n + c;
    }, 0),
  );

  /**
   * Format and duplicate errors show as soon as they occur; "missing value" errors wait until the
   * user tries to leave the step, so a fresh form isn't covered in red.
   */
  function err(key: string): string | undefined {
    const msg = errors[key];
    if (!msg) return undefined;
    const missing = /required|^Select|^Add at least/.test(msg);
    return !missing || attempted[w.step] ? msg : undefined;
  }

  function stepState(i: number): "current" | "done" | "error" | "todo" {
    if (i === w.step) return "current";
    if (attempted[i] && Object.keys(stepErrors(w, i)).length) return "error";
    return i < w.step ? "done" : "todo";
  }

  export async function focusFirst() {
    await tick();
    body
      ?.querySelector<HTMLElement>(
        "input:not([type=checkbox]):not([type=radio]):not(:disabled), textarea, select, .empty-add",
      )
      ?.focus();
  }

  export function goTo(step: number) {
    if (step > w.step) {
      // Moving forward requires every step before the target to be valid.
      for (let s = w.step; s < step; s++) {
        if (Object.keys(stepErrors(w, s)).length) {
          attempted[s] = true;
          w.step = s;
          focusFirstError();
          return;
        }
      }
    }
    w.step = Math.min(Math.max(step, 1), LAST);
    body?.closest(".body")?.scrollTo({ top: 0 });
    focusFirst();
  }

  /** Advance one step. Returns false when already on the last step (caller should create). */
  export function next(): boolean {
    if (w.step >= LAST) return false;
    attempted[w.step] = true;
    if (Object.keys(errors).length) {
      focusFirstError();
      return true;
    }
    goTo(w.step + 1);
    return true;
  }

  export function back() {
    if (w.step > 1) goTo(w.step - 1);
  }

  /** Before creating: jump to the first invalid step and reveal its errors. */
  export function ensureValid(): boolean {
    const bad = firstInvalidStep(w);
    if (bad === null) return true;
    attempted[bad] = true;
    w.step = bad;
    focusFirstError();
    return false;
  }

  async function focusFirstError() {
    await tick();
    const el = body?.querySelector<HTMLElement>("[role=alert]");
    const card = el?.closest("details");
    if (card && !card.open) card.open = true;
    await tick();
    el?.closest(".field")?.querySelector<HTMLElement>("input, select, button")?.focus();
  }

  async function copy() {
    try {
      await navigator.clipboard.writeText(specJson);
      toast.success("Spec copied to clipboard");
    } catch (e) {
      toast.error(e);
    }
  }

  // Server-side dry run on the review step (debounced; keeps the last report while re-checking).
  let report = $state<ValidationReport | null>(null);
  let validating = $state(false);
  $effect(() => {
    if (w.step !== LAST) return;
    const spec = specJson;
    const t = setTimeout(async () => {
      validating = true;
      try {
        report = await validateSpec({ spec, is_file: false, file_extension: ".json" });
      } catch (e) {
        report = { valid: false, errors: [String(e)], warnings: [] };
      } finally {
        validating = false;
      }
    }, 250);
    return () => clearTimeout(t);
  });
</script>

<div class="wizard">
  <ol class="steps" aria-label="Wizard steps">
    {#each STEPS as label, i (label)}
      {@const n = i + 1}
      {@const st = stepState(n)}
      <li class={st}>
        <button
          type="button"
          onclick={() => goTo(n)}
          aria-current={st === "current" ? "step" : undefined}
          title="Step {n}: {label}"
        >
          <span class="num">
            {#if st === "done"}<Icon name="check" size={12} />{:else if st === "error"}!{:else}{n}{/if}
          </span>
          <span class="label">{label}</span>
        </button>
      </li>
    {/each}
  </ol>

  <div class="cols" class:with-preview={showPreview && w.step !== LAST}>
    <section class="step" bind:this={body}>
      {#if w.step === 1}
        <div class="form">
          <label class="field" class:invalid={!!err("name")}>
            <span>Workflow name <span class="req">*</span></span>
            <input class="input" bind:value={w.name} placeholder="my-data-pipeline" />
            {#if err("name")}<span class="err" role="alert">{err("name")}</span>{/if}
          </label>
          <label class="field">
            <span>Description</span>
            <input
              class="input"
              bind:value={w.description}
              placeholder="Optional: what does this workflow do?"
            />
          </label>
          <div class="field" role="radiogroup" aria-label="Parallelization strategy">
            <span>Parallelization strategy</span>
            <div class="strategies">
              <label class="strategy" class:selected={w.strategy === "resource_aware"}>
                <input type="radio" bind:group={w.strategy} value="resource_aware" />
                <strong>Resource-aware <span class="tag">Recommended</span></strong>
                <small>
                  Define per-job CPU, memory, and GPU needs. The runner packs jobs onto available
                  resources.
                </small>
              </label>
              <label class="strategy" class:selected={w.strategy === "queue_depth"}>
                <input type="radio" bind:group={w.strategy} value="queue_depth" />
                <strong>Queue-depth</strong>
                <small>
                  Cap concurrent jobs with <code>--max-parallel-jobs</code>. Best for homogeneous
                  jobs.
                </small>
              </label>
            </div>
            <span class="hint">
              <a
                href="https://natlabrockies.github.io/torc/latest/core/concepts/parallelization.html"
                target="_blank"
                rel="noreferrer">Learn more about parallelization</a
              >
            </span>
          </div>
        </div>
      {:else if w.step === 2}
        <WizardJobs bind:w {err} />
      {:else if w.step === 3}
        <WizardSchedulers bind:w {err} slurmAuto={!!slurm} />
      {:else if w.step === 4}
        <WizardActions bind:w {err} slurmAuto={!!slurm} />
      {:else if w.step === 5}
        <div class="form">
          <div>
            <h4>Resource monitoring</h4>
            <p class="muted sub">Track CPU, memory, and process usage while jobs run.</p>
          </div>
          <label class="check">
            <input type="checkbox" bind:checked={w.monitor.enabled} />
            Enable resource monitoring
          </label>
          {#if w.monitor.enabled}
            <div class="field" role="radiogroup" aria-label="Granularity">
              <span>Granularity</span>
              <div class="strategies">
                <label class="strategy" class:selected={w.monitor.granularity === "summary"}>
                  <input type="radio" bind:group={w.monitor.granularity} value="summary" />
                  <strong>Summary</strong>
                  <small>Peak and average values only.</small>
                </label>
                <label class="strategy" class:selected={w.monitor.granularity === "time_series"}>
                  <input type="radio" bind:group={w.monitor.granularity} value="time_series" />
                  <strong>Time series</strong>
                  <small>Samples at regular intervals. More detail, more storage.</small>
                </label>
              </div>
            </div>
            <label class="field narrow" class:invalid={!!err("interval")}>
              <span>Sample interval (seconds)</span>
              <input
                class="input"
                type="number"
                min="1"
                max="300"
                bind:value={w.monitor.sample_interval_seconds}
              />
              {#if err("interval")}
                <span class="err" role="alert">{err("interval")}</span>
              {:else}
                <span class="hint">How often to sample resource usage (1–300).</span>
              {/if}
            </label>
          {/if}
        </div>
      {:else}
        <div class="review">
          <div class="summary">
            <div><span class="v">{w.name || "untitled-workflow"}</span><span class="l">Name</span></div>
            <div>
              <span class="v">{w.jobs.length}</span>
              <span class="l">Job specs{#if totalJobs !== null && totalJobs !== w.jobs.length}
                  → {totalJobs} jobs{/if}</span>
            </div>
            <div><span class="v">{w.schedulers.length}</span><span class="l">Schedulers</span></div>
            <div><span class="v">{w.actions.length}</span><span class="l">Actions</span></div>
            <div>
              <span class="v">{w.monitor.enabled ? w.monitor.granularity.replace("_", " ") : "off"}</span>
              <span class="l">Monitoring</span>
            </div>
          </div>
          {#if slurm}
            <div class="note">
              Slurm schedulers and actions will be generated when the workflow is created, based on
              job resource requirements (account <code>{slurm.account || "(not set)"}</code>, profile
              <code>{slurm.profile ?? "auto-detected"}</code>).
            </div>
          {/if}
          <ValidationPanel {report} busy={validating} />
          <div class="preview-head">
            <strong>Generated spec</strong>
            <button type="button" class="btn btn-sm btn-ghost" onclick={copy}>Copy JSON</button>
          </div>
          <pre class="code-block big hl">{@html highlightJson(specJson)}</pre>
        </div>
      {/if}
    </section>

    {#if w.step !== LAST}
      {#if showPreview}
        <aside class="preview">
          <div class="preview-head">
            <strong>Live spec</strong>
            <span class="grow"></span>
            <button type="button" class="btn btn-sm btn-ghost" onclick={copy}>Copy</button>
            <button
              type="button"
              class="btn btn-sm btn-ghost btn-icon"
              aria-label="Hide preview"
              title="Hide preview"
              onclick={() => (showPreview = false)}><Icon name="x" size={14} /></button
            >
          </div>
          <pre class="code-block hl">{@html highlightJson(specJson)}</pre>
        </aside>
      {:else}
        <button type="button" class="show-preview btn btn-sm btn-ghost" onclick={() => (showPreview = true)}>
          Show spec preview
        </button>
      {/if}
    {/if}
  </div>
</div>

<style>
  .wizard {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .steps {
    display: flex;
    gap: 4px;
    margin: 0;
    padding: 0;
    list-style: none;
    counter-reset: step;
  }
  .steps li {
    flex: 1;
    position: relative;
  }
  .steps li:not(:last-child)::after {
    content: "";
    position: absolute;
    top: 50%;
    right: -4px;
    width: 4px;
    height: 1px;
    background: var(--border-strong);
  }
  .steps button {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    color: var(--text-muted);
    font: inherit;
    cursor: pointer;
    transition:
      background 0.12s,
      border-color 0.12s;
  }
  .steps button:hover {
    background: var(--surface-hover);
  }
  .steps button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .num {
    display: inline-grid;
    place-items: center;
    flex-shrink: 0;
    width: 22px;
    height: 22px;
    border-radius: 999px;
    background: var(--neutral-soft);
    font-size: 12px;
    font-weight: 600;
  }
  .label {
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .current button {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--text);
  }
  .current .num {
    background: var(--accent);
    color: var(--accent-text);
  }
  .done .num {
    background: var(--success-soft);
    color: var(--success);
  }
  .done button {
    color: var(--text);
  }
  .error button {
    border-color: var(--danger);
  }
  .error .num {
    background: var(--danger);
    color: var(--accent-text);
  }

  .cols {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 20px;
    align-items: start;
  }
  .cols.with-preview {
    grid-template-columns: minmax(0, 1fr) minmax(280px, 38%);
  }
  .step {
    min-width: 0;
    animation: fade 0.14s ease-out;
  }
  .preview {
    position: sticky;
    top: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .preview .code-block {
    max-height: calc(100vh - 330px);
    min-height: 200px;
    font-size: 11.5px;
    white-space: pre;
  }
  .preview-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    font-size: 13px;
  }
  .show-preview {
    justify-self: end;
  }
  @media (max-width: 900px) {
    .cols.with-preview {
      grid-template-columns: minmax(0, 1fr);
    }
    .preview {
      position: static;
    }
    .label {
      display: none;
    }
    .current .label {
      display: inline;
    }
  }

  .form {
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 640px;
  }
  .sub {
    margin: 2px 0 0;
    font-size: 12.5px;
  }
  .narrow {
    max-width: 240px;
  }
  .req,
  .err {
    color: var(--danger);
  }
  .err {
    font-size: 12px;
    font-weight: 400;
  }
  .invalid .input {
    border-color: var(--danger);
  }
  .strategies {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .strategy {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 10px 12px 10px 34px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    cursor: pointer;
    font-weight: 400;
  }
  .strategy input {
    position: absolute;
    left: 12px;
    top: 12px;
    margin: 0;
    accent-color: var(--accent);
  }
  .strategy small {
    color: var(--text-muted);
    font-size: 12px;
  }
  .strategy.selected {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .strategy:has(input:focus-visible) {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .tag {
    margin-left: 4px;
    padding: 0 6px;
    border-radius: 999px;
    background: var(--success-soft);
    color: var(--success);
    font-size: 11px;
    font-weight: 500;
  }

  .review {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .summary {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }
  .summary div {
    display: flex;
    flex-direction: column;
    padding: 8px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface-2);
    min-width: 110px;
  }
  .summary .v {
    font-weight: 600;
    font-size: 15px;
    text-transform: capitalize;
  }
  .summary div:first-child .v {
    text-transform: none;
  }
  .summary .l {
    color: var(--text-muted);
    font-size: 12px;
  }
  .note {
    padding: 10px 12px;
    border-radius: var(--radius);
    background: var(--info-soft);
    color: var(--text);
    font-size: 13px;
  }
  .big {
    white-space: pre;
    max-height: 420px;
  }
  @keyframes fade {
    from {
      opacity: 0;
      transform: translateY(3px);
    }
  }
</style>
