<!--
  Create-workflow modal: upload a spec file, point at a server-side path, paste a spec, or build one
  with the wizard. Optional Slurm scheduler generation and initialize-after-create.
-->
<script lang="ts">
  import { tick } from "svelte";
  import { app } from "../../lib/state.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import Modal from "../../lib/ui/Modal.svelte";
  import SegmentedTabs from "../../lib/ui/SegmentedTabs.svelte";
  import { toast } from "../../lib/ui/toast.svelte";
  import { formatBytes } from "../../lib/format";
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
  import ValidationPanel from "./ValidationPanel.svelte";
  import Wizard from "./Wizard.svelte";

  type Source = "upload" | "path" | "inline" | "wizard";
  const SOURCES: { id: Source; label: string }[] = [
    { id: "upload", label: "Upload file" },
    { id: "path", label: "File path" },
    { id: "inline", label: "Paste spec" },
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

  let initialize = $state(true);
  let profiles = $state<HpcProfiles | null>(null);
  let useSlurm = $state(false);
  let slurmAccount = $state("");
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
  const slurmActive = $derived(useSlurm && !!detectedProfile);
  const onLastWizardStep = $derived(source === "wizard" && wizard.step === STEPS.length);

  // On open: check HPC profiles (as the legacy showModal did) and focus the first control.
  $effect(() => {
    if (!app.createOpen) return;
    getHpcProfiles()
      .then((p) => {
        profiles = p;
        if (!p.detected_profile) useSlurm = false;
      })
      .catch(() => (profiles = null));
    focusFirst();
  });

  // Stale results are misleading once the input changes.
  $effect(() => {
    void [source, upload, path, inline, inlineFormat];
    report = null;
    createError = null;
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
      case "wizard":
        return {
          spec: JSON.stringify(generateSpec(wizard), null, 2),
          is_file: false,
          file_extension: ".json",
        };
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
      const slurm = slurmActive ? { account: slurmAccount.trim(), profile: detectedProfile } : null;
      const id = await createWorkflow(body, slurm);
      toast.success(id !== null ? `Created workflow ${id}` : "Workflow created");
      // Clear the form for next time.
      upload = null;
      path = "";
      inline = "";
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
        <kbd>←</kbd><kbd>→</kbd> or <kbd>1</kbd>–<kbd>4</kbd> switch source
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
              <button
                type="button"
                class="btn btn-sm btn-ghost"
                onclick={() => {
                  upload = null;
                  if (fileInput) fileInput.value = "";
                }}>Clear</button
              >
            </div>
            <pre class="code-block">{upload.content}</pre>
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
      {:else}
        <Wizard bind:this={wizardRef} bind:w={wizard} slurm={slurmActive ? { account: slurmAccount, profile: detectedProfile } : null} />
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
          <label class="check" class:disabled={!detectedProfile}>
            <input type="checkbox" bind:checked={useSlurm} disabled={!detectedProfile} />
            Generate Slurm schedulers
            {#if detectedProfile}<span class="pill">{detectedProfile}</span>{/if}
          </label>
          {#if !detectedProfile}
            <span class="faint small">No HPC profile detected on this machine; Slurm generation is unavailable.</span>
          {:else if useSlurm}
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
  .pill {
    padding: 0 8px;
    border-radius: 999px;
    background: var(--info-soft);
    color: var(--info);
    font-size: 11.5px;
    font-weight: 500;
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
