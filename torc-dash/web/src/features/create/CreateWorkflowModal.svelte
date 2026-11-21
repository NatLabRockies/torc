<!--
  Create-workflow modal: upload a spec file, point at a server-side path, paste a spec, list
  commands to run, or build one with the wizard. Optional Slurm scheduler generation (with an
  allocation-plan preview) and initialize-after-create.
-->
<script lang="ts">
  import { highlightByName, prettyJson } from "../../lib/highlight";
  import { tick } from "svelte";
  import { app } from "../../lib/state.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import Modal from "../../lib/ui/Modal.svelte";
  import SegmentedTabs from "../../lib/ui/SegmentedTabs.svelte";
  import { toast } from "../../lib/ui/toast.svelte";
  import { op } from "../../lib/api/dash";
  import { errorText, formatBytes } from "../../lib/format";
  import {
    createWorkflow,
    getHpcProfiles,
    validateSpec,
    type HpcProfiles,
    type SpecBody,
    type ValidationReport,
  } from "./actions";
  import { initializeWorkflow } from "../workflows/actions";
  import { STEPS, generateSpec, newWizard } from "./spec";
  import { draft } from "../editor/draft.svelte";
  import ValidationPanel from "./ValidationPanel.svelte";
  import AllocationPlan, { type Plan } from "./AllocationPlan.svelte";
  import { commandsSpec } from "./commands";
  import Wizard from "./Wizard.svelte";

  type Source = "upload" | "path" | "inline" | "commands" | "wizard";
  const SOURCES: { id: Source; label: string }[] = [
    { id: "upload", label: "Upload file" },
    { id: "path", label: "File path" },
    { id: "inline", label: "Paste spec" },
    { id: "commands", label: "Commands" },
    { id: "wizard", label: "Wizard" },
  ];
  const FORMATS = [
    { ext: "auto", label: "Auto-detect" },
    { ext: ".yaml", label: "YAML" },
    { ext: ".json", label: "JSON" },
    { ext: ".json5", label: "JSON5" },
    { ext: ".kdl", label: "KDL" },
  ];

  let source = $state<Source>("upload");
  let upload = $state<{ name: string; size: number; content: string; ext: string } | null>(null);
  let dragging = $state(false);
  let path = $state("");
  let inline = $state("");
  let inlineFormat = $state("auto");
  let wizard = $state(newWizard());
  let commands = $state("");
  let commandsName = $state("");

  let initialize = $state(true);
  let profiles = $state<HpcProfiles | null>(null);
  let useSlurm = $state(false);
  /** HPC profile for Slurm generation: the detected one by default, or any known profile. */
  let profile = $state<string | null>(null);
  let slurmAccount = $state("");
  let plan = $state<Plan | null>(null);
  let planning = $state(false);
  let planError = $state<string | null>(null);
  let accountError = $state(false);

  let busy = $state(false);
  let validating = $state(false);
  let report = $state<ValidationReport | null>(null);
  let createError = $state<string | null>(null);

  let root = $state<HTMLElement>();
  let fileInput = $state<HTMLInputElement>();
  let wizardRef = $state<ReturnType<typeof Wizard>>();

  const detectedProfile = $derived(profiles?.detected_profile ?? null);
  const slurmAvailable = $derived(!!profiles?.success && (profiles.profiles?.length ?? 0) > 0);
  const slurmActive = $derived(useSlurm && !!profile);
  const onLastWizardStep = $derived(source === "wizard" && wizard.step === STEPS.length);

  // On open: check HPC profiles (as the legacy showModal did) and focus the first control.
  $effect(() => {
    if (!app.createOpen) return;
    getHpcProfiles()
      .then((p) => {
        profiles = p;
        if (!profile || !p.profiles?.some((x) => x.name === profile)) {
          profile = p.detected_profile ?? p.profiles?.[0]?.name ?? null;
        }
        if (!profile) useSlurm = false;
      })
      .catch(() => (profiles = null));
    focusFirst();
  });

  // Stale results are misleading once the input changes.
  $effect(() => {
    void [source, upload, path, inline, inlineFormat, commands, commandsName];
    report = null;
    createError = null;
    plan = null;
    planError = null;
  });
  $effect(() => {
    void [profile, slurmAccount];
    plan = null;
  });

  function onClose() {
    // Matches legacy: closing resets the wizard; other inputs are kept until a successful create.
    wizard = newWizard();
    report = null;
    createError = null;
    accountError = false;
  }

  function close() {
    app.createOpen = false;
  }

  /** Hand a spec to the spec editor tab; JSON (from the wizard or an upload) becomes YAML. */
  async function toEditor(text: string, json = false) {
    if (json) text = (await import("yaml")).stringify(JSON.parse(text));
    close();
    if (!(await draft.open(text))) app.createOpen = true;
  }

  async function readFile(file: File) {
    try {
      const content = await file.text();
      const dot = file.name.lastIndexOf(".");
      upload = {
        name: file.name,
        size: file.size,
        content,
        ext: dot >= 0 ? file.name.slice(dot).toLowerCase() : ".json",
      };
    } catch (e) {
      toast.error(e);
    }
  }

  // Highlighted preview of the uploaded spec; JSON is re-indented for display only.
  let uploadHtml = $derived(
    upload
      ? highlightByName(upload.ext === ".json" ? prettyJson(upload.content) : upload.content, upload.ext)
      : null,
  );

  function detectFormat(text: string): string {
    const t = text.trim();
    try {
      JSON.parse(t);
      return ".json";
    } catch {
      // not strict JSON
    }
    if (t.startsWith("{")) return ".json5";
    if (/^\s*(name|description|job|file)\s+"/m.test(t) && !/^\s*\w+:\s/m.test(t)) return ".kdl";
    return ".yaml";
  }
  const detected = $derived(inline.trim() ? detectFormat(inline) : null);

  /** Request body for the current source, or an error message. */
  function specBody(): SpecBody | string {
    switch (source) {
      case "upload":
        return upload
          ? { spec: upload.content, is_file: false, file_extension: upload.ext }
          : "Choose a workflow spec file to upload";
      case "path":
        return path.trim() ? { spec: path.trim(), is_file: true } : "Enter a spec file path";
      case "inline": {
        if (!inline.trim()) return "Paste a workflow spec";
        const ext = inlineFormat === "auto" ? detectFormat(inline) : inlineFormat;
        return { spec: inline, is_file: false, file_extension: ext };
      }
      case "commands": {
        const spec = commandsSpec(commands, commandsName);
        return spec
          ? { spec: JSON.stringify(spec, null, 2), is_file: false, file_extension: ".json" }
          : "Enter at least one command";
      }
      case "wizard":
        return {
          spec: JSON.stringify(generateSpec(wizard), null, 2),
          is_file: false,
          file_extension: ".json",
        };
    }
  }

  /** Recommend Slurm allocations for the spec (torc slurm plan-allocations). */
  async function previewPlan() {
    if (source === "wizard" && !wizardRef?.ensureValid()) return;
    const body = specBody();
    if (typeof body === "string") return toast.info(body);
    planning = true;
    planError = null;
    try {
      plan = await op<Plan>("plan-allocations", {
        ...(body.is_file ? { spec_path: body.spec } : { spec: body.spec, file_extension: body.file_extension }),
        account: slurmAccount.trim() || undefined,
        profile,
        // Live sinfo/squeue only make sense on the cluster the profile describes.
        offline: profile !== detectedProfile,
      });
    } catch (e) {
      planError = errorText(e);
    } finally {
      planning = false;
    }
  }

  async function validate() {
    if (source === "wizard" && !wizardRef?.ensureValid()) return;
    const body = specBody();
    if (typeof body === "string") return toast.info(body);
    validating = true;
    try {
      report = await validateSpec(body);
    } catch (e) {
      toast.error(e);
    } finally {
      validating = false;
    }
  }

  async function create() {
    if (busy) return;
    if (source === "wizard" && !wizardRef?.ensureValid()) return;
    const body = specBody();
    if (typeof body === "string") return toast.info(body);
    if (slurmActive && !slurmAccount.trim()) {
      accountError = true;
      root?.querySelector<HTMLInputElement>("#create-slurm-account")?.focus();
      return;
    }
    busy = true;
    createError = null;
    try {
      const slurm = slurmActive ? { account: slurmAccount.trim(), profile } : null;
      const id = await createWorkflow(body, slurm);
      toast.success(id !== null ? `Created workflow ${id}` : "Workflow created");
      // Clear the form for next time.
      upload = null;
      path = "";
      inline = "";
      commands = "";
      commandsName = "";
      if (fileInput) fileInput.value = "";
      close();
      app.refresh();
      if (id !== null) {
        app.selectWorkflow(id);
        if (initialize) await initializeWorkflow(id);
      }
    } catch (e) {
      createError = e instanceof Error ? e.message : String(e);
      toast.error("Failed to create workflow");
    } finally {
      busy = false;
    }
  }

  /** Primary button: Next on wizard steps, Create everywhere else. */
  function primary() {
    if (source === "wizard" && wizardRef?.next()) return;
    create();
  }

  async function focusFirst() {
    await tick();
    if (source === "wizard") return wizardRef?.focusFirst();
    root
      ?.querySelector<HTMLElement>(
        `[data-panel="${source}"] :is(.dropzone, input:not([type=file]), textarea)`,
      )
      ?.focus();
  }

  function switchSource(s: Source) {
    source = s;
    focusFirst();
  }

  const isTyping = (t: EventTarget | null) =>
    t instanceof HTMLElement && !!t.closest("input, textarea, select, [contenteditable]");

  // Keyboard shortcuts inside the modal (port of legacy handleCreateModalKeyboard).
  function onKeydown(e: KeyboardEvent) {
    if (!app.createOpen || e.defaultPrevented || !root) return;
    const dialog = (e.target as Element | null)?.closest?.("dialog");
    if (!dialog?.contains(root)) return;

    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      primary();
      return;
    }
    if (e.metaKey || e.ctrlKey || e.altKey || isTyping(e.target)) return;

    const i = SOURCES.findIndex((s) => s.id === source);
    const n = SOURCES.length;
    const k = e.key;
    if (k === "ArrowLeft" || k === "ArrowRight") {
      switchSource(SOURCES[(i + (k === "ArrowLeft" ? n - 1 : 1)) % n].id);
    } else if (k >= "1" && k <= String(n)) {
      switchSource(SOURCES[Number(k) - 1].id);
    } else if (source === "wizard" && (k === "[" || k === "ArrowUp")) {
      wizardRef?.back();
    } else if (source === "wizard" && (k === "]" || k === "ArrowDown")) {
      wizardRef?.next();
    } else if (k === "Enter" && !(e.target as Element).closest("button, a, summary, label")) {
      if (onLastWizardStep) create();
      else focusFirst();
    } else return;
    e.preventDefault();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<Modal bind:open={app.createOpen} title="Create workflow" size="xl" onclose={onClose}>
  <div class="create" bind:this={root}>
    <div class="top">
      <SegmentedTabs
        bind:value={() => source, (s) => switchSource(s)}
        tabs={SOURCES}
      />
      <span class="faint hint">
        <kbd>←</kbd><kbd>→</kbd> or <kbd>1</kbd>–<kbd>{SOURCES.length}</kbd> switch source
      </span>
    </div>

    <div class="panel" data-panel={source}>
      {#if source === "upload"}
        <button
          type="button"
          class="dropzone"
          class:dragging
          class:has-file={!!upload}
          onclick={() => fileInput?.click()}
          ondragover={(e) => {
            e.preventDefault();
            dragging = true;
          }}
          ondragleave={() => (dragging = false)}
          ondrop={(e) => {
            e.preventDefault();
            dragging = false;
            const f = e.dataTransfer?.files[0];
            if (f) readFile(f);
          }}
        >
          <Icon name={upload ? "file" : "upload"} size={28} />
          {#if upload}
            <strong>{upload.name}</strong>
            <span class="muted">{formatBytes(upload.size)} · click or drop to replace</span>
          {:else}
            <strong>Drop a workflow spec here, or click to choose</strong>
            <span class="muted">From this computer · YAML, JSON, JSON5, or KDL</span>
          {/if}
        </button>
        <input
          bind:this={fileInput}
          type="file"
          accept=".yaml,.yml,.json,.json5,.kdl"
          hidden
          onchange={(e) => {
            const f = e.currentTarget.files?.[0];
            if (f) readFile(f);
          }}
        />
        {#if upload}
          <div class="upload-preview">
            <div class="row">
              <strong>Contents</strong>
              <span class="grow"></span>
              <button
                type="button"
                class="btn btn-sm btn-ghost"
                onclick={() => {
                  inline = upload?.content ?? "";
                  inlineFormat = upload?.ext === ".yml" ? ".yaml" : (upload?.ext ?? "auto");
                  switchSource("inline");
                }}>Edit as pasted spec</button
              >
              {#if /^\.(ya?ml|json)$/.test(upload.ext)}
                <button
                  type="button"
                  class="btn btn-sm btn-ghost"
                  onclick={() =>
                    upload &&
                    toEditor(upload.content, upload.ext === ".json")}
                  >Open in spec editor</button
                >
              {/if}
              <button
                type="button"
                class="btn btn-sm btn-ghost"
                onclick={() => {
                  upload = null;
                  if (fileInput) fileInput.value = "";
                }}>Clear</button
              >
            </div>
            <!-- highlightByName escapes its input. -->
            {#if uploadHtml !== null}<pre class="code-block hl">{@html uploadHtml}</pre>
            {:else}<pre class="code-block">{upload.content}</pre>{/if}
          </div>
        {/if}
      {:else if source === "path"}
        <form
          class="field"
          onsubmit={(e) => {
            e.preventDefault();
            create();
          }}
        >
          <label for="create-spec-path">Spec file path</label>
          <input
            id="create-spec-path"
            class="input mono"
            bind:value={path}
            placeholder="/path/to/workflow.yaml"
            autocomplete="off"
          />
          <span class="hint">
            A path on the machine running torc-dash (relative paths resolve from its working
            directory).
          </span>
        </form>
      {:else if source === "inline"}
        <div class="field grow-field">
          <div class="row">
            <label for="create-spec-text">Workflow spec</label>
            <span class="grow"></span>
            <select class="select sm" bind:value={inlineFormat} aria-label="Spec format">
              {#each FORMATS as f (f.ext)}
                <option value={f.ext}>
                  {f.label}{#if f.ext === "auto" && detected}
                    ({FORMATS.find((x) => x.ext === detected)?.label}){/if}
                </option>
              {/each}
            </select>
          </div>
          <textarea
            id="create-spec-text"
            class="input editor"
            bind:value={inline}
            spellcheck="false"
            placeholder={'name: my-workflow\njobs:\n  - name: hello\n    command: echo "hello"'}
            onkeydown={(e) => {
              // Tab inserts two spaces instead of leaving the editor (Esc then Tab still moves on).
              if (e.key === "Tab" && !e.shiftKey && !e.metaKey && !e.ctrlKey) {
                e.preventDefault();
                const t = e.currentTarget;
                t.setRangeText("  ", t.selectionStart, t.selectionEnd, "end");
                inline = t.value;
              }
            }}
          ></textarea>
        </div>
      {:else if source === "commands"}
        <label class="field">
          Workflow name
          <input class="input" bind:value={commandsName} placeholder="commands" autocomplete="off" />
        </label>
        <div class="field grow-field">
          <label for="create-commands">Commands, one per line</label>
          <textarea
            id="create-commands"
            class="input editor"
            bind:value={commands}
            spellcheck="false"
            placeholder={"python train.py --seed 1\npython train.py --seed 2\n# Lines starting with # are skipped"}
          ></textarea>
          <span class="hint">
            Each line becomes an independent job (job_001, job_002, …) with default resources. Use
            the wizard or a spec for dependencies and resource requirements.
          </span>
        </div>
      {:else}
        <Wizard bind:this={wizardRef} bind:w={wizard} slurm={slurmActive ? { account: slurmAccount, profile } : null} />
      {/if}
    </div>

    {#if source !== "wizard"}
      <ValidationPanel {report} busy={validating} />
    {/if}
    {#if createError}
      <div class="create-error" role="alert">
        <strong>Create failed</strong>
        <pre>{createError}</pre>
      </div>
    {/if}

    <fieldset class="options">
      <legend>Options</legend>
      <label class="check">
        <input type="checkbox" bind:checked={initialize} />
        Initialize workflow after creation
      </label>
      {#if slurmAvailable}
        <div class="slurm">
          <div class="row">
            <label class="check" class:disabled={!profile}>
              <input type="checkbox" bind:checked={useSlurm} disabled={!profile} />
              Generate Slurm schedulers for
            </label>
            <select class="select sm" bind:value={profile} aria-label="HPC profile">
              {#each profiles?.profiles ?? [] as p (p.name)}
                <option value={p.name}>{p.display_name}{p.is_detected ? " (this system)" : ""}</option>
              {/each}
            </select>
          </div>
          {#if !detectedProfile}
            <span class="faint small">
              No HPC system detected here; generation uses the chosen profile's partitions.
            </span>
          {/if}
          {#if useSlurm}
            <label class="field account" class:invalid={accountError && !slurmAccount.trim()}>
              <span>Slurm account <span class="req">*</span></span>
              <input
                id="create-slurm-account"
                class="input"
                bind:value={slurmAccount}
                placeholder="e.g. myproject"
                oninput={() => (accountError = false)}
              />
              {#if accountError && !slurmAccount.trim()}
                <span class="err" role="alert">Enter the Slurm account to charge</span>
              {:else}
                <span class="hint">Used for all generated job submissions.</span>
              {/if}
            </label>
            <div class="row">
              <button type="button" class="btn btn-sm" onclick={previewPlan} disabled={planning}>
                {#if planning}<span class="spinner"></span>{/if} Preview allocation plan
              </button>
              {#if planError}<span class="error-text small">{planError}</span>{/if}
            </div>
            {#if plan}<AllocationPlan {plan} />{/if}
          {/if}
        </div>
      {/if}
    </fieldset>
  </div>

  {#snippet footer()}
    {#if source === "wizard"}
      <button type="button" class="btn" onclick={() => wizardRef?.back()} disabled={wizard.step === 1}>
        Back
      </button>
      <span class="faint keys"><kbd>[</kbd> <kbd>]</kbd> steps</span>
      {#if onLastWizardStep}
        <button type="button" class="btn btn-ghost" onclick={() => toEditor(JSON.stringify(generateSpec(wizard)), true)}>
          Edit as YAML
        </button>
      {/if}
    {/if}
    <span class="grow"></span>
    <span class="faint keys"><kbd>{navigator.platform.includes("Mac") ? "⌘" : "Ctrl"}</kbd><kbd>↵</kbd></span>
    <button type="button" class="btn" onclick={validate} disabled={validating || busy}>
      {#if validating}<span class="spinner"></span>{/if} Validate
    </button>
    <button type="button" class="btn" onclick={close}>Cancel</button>
    <button type="button" class="btn btn-primary" onclick={primary} disabled={busy}>
      {#if busy}<span class="spinner"></span> Creating…{:else if source === "wizard" && !onLastWizardStep}
        Next <Icon name="chevronRight" size={14} />
      {:else}<Icon name="plus" size={14} /> Create{/if}
    </button>
  {/snippet}
</Modal>

<style>
  .create {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-height: 100%;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  .hint {
    font-size: 12px;
  }
  .panel {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .mono {
    font-family: var(--mono);
  }
  .dropzone {
    display: grid;
    place-items: center;
    gap: 6px;
    padding: 48px 16px;
    border: 2px dashed var(--border-strong);
    border-radius: var(--radius-lg);
    background: var(--surface-2);
    color: var(--text-muted);
    font: inherit;
    cursor: pointer;
    transition:
      border-color 0.12s,
      background 0.12s;
  }
  .dropzone strong {
    color: var(--text);
  }
  .dropzone:hover,
  .dropzone:focus-visible,
  .dropzone.dragging {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent);
    outline: none;
  }
  .dropzone.has-file {
    border-style: solid;
    padding: 24px 16px;
  }
  .upload-preview {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .upload-preview .code-block {
    max-height: 320px;
    white-space: pre;
  }
  .grow-field {
    flex: 1;
  }
  .select.sm {
    height: 26px;
    font-size: 12.5px;
  }
  .editor {
    flex: 1;
    min-height: 340px;
    font-family: var(--mono);
    font-size: 12.5px;
    line-height: 1.5;
    tab-size: 2;
    white-space: pre;
  }
  .create-error {
    padding: 10px 14px;
    border: 1px solid var(--danger);
    border-radius: var(--radius);
    background: var(--danger-soft);
    color: var(--danger);
  }
  .create-error pre {
    margin: 6px 0 0;
    font-size: 12.5px;
    white-space: pre-wrap;
    word-break: break-word;
    max-height: 200px;
    overflow: auto;
  }
  .options {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: 10px 28px;
    margin: 0;
    padding: 10px 14px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }
  legend {
    padding: 0 6px;
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .slurm {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .check.disabled {
    color: var(--text-faint);
    cursor: default;
  }
  .small {
    font-size: 12px;
  }
  .account {
    max-width: 280px;
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
  .grow {
    flex: 1;
  }
  .keys {
    font-size: 12px;
    display: inline-flex;
    align-items: center;
    gap: 2px;
  }
  @media (max-width: 700px) {
    .keys,
    .hint {
      display: none;
    }
  }
</style>
