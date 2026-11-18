<!--
  Workflow spec editor: YAML with schema completion, hover docs, and lint; templates from
  examples/yaml; snippets for common blocks; open/save files on the torc-dash host; validate and
  create.
-->
<script lang="ts">
  import { parse, stringify } from "yaml";
  import { dashGet, dashPost } from "../../lib/api/dash";
  import { errorText } from "../../lib/format";
  import { highlightYaml } from "../../lib/highlight";
  import { app } from "../../lib/state.svelte";
  import { confirm } from "../../lib/ui/confirm.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import Modal from "../../lib/ui/Modal.svelte";
  import { toast } from "../../lib/ui/toast.svelte";
  import { createWorkflow, validateSpec, type ValidationReport } from "../create/actions";
  import ValidationPanel from "../create/ValidationPanel.svelte";
  import { initializeWorkflow } from "../workflows/actions";
  import { draft } from "./draft.svelte";
  import { SNIPPETS, insertSnippet } from "./snippets";
  import SpecEditor from "./SpecEditor.svelte";
  import { BLANK_TEMPLATE, loadTemplates, type Template } from "./templates";
  import { SpecSchema, type Schema } from "./yamlSchema";

  // An older torc-dash has no schema endpoint; the editor then works without completion and lint.
  const schema = dashGet<Schema>("/api/workflow-schema")
    .then((s) => new SpecSchema(s))
    .catch(() => null);

  let editor = $state<ReturnType<typeof SpecEditor>>();
  let report = $state<ValidationReport | null>(null);
  let validating = $state(false);
  let creating = $state(false);
  let initialize = $state(true);

  let templatesOpen = $state(false);
  let templates = $state<Template[] | null>(null);
  const allTemplates = $derived([BLANK_TEMPLATE, ...(templates ?? [])]);
  let picked = $state<Template | null>(null);
  const preview = $derived(picked ? highlightYaml(picked.text) : "");

  let pathDialog = $state<"open" | "save" | null>(null);
  let pathInput = $state("");
  let pathBusy = $state(false);
  let fileInput = $state<HTMLInputElement>();

  const insertId = $props.id();
  let insertTrigger = $state<HTMLButtonElement>();
  let insertMenu = $state<HTMLDivElement>();

  // Results describe the text they were run on.
  $effect(() => {
    void draft.text;
    report = null;
  });

  async function showTemplates() {
    templatesOpen = true;
    picked ??= BLANK_TEMPLATE;
    templates ??= await loadTemplates();
  }

  async function useTemplate(text: string) {
    templatesOpen = false;
    if (!(await draft.open(text))) templatesOpen = true;
  }

  function placeInsertMenu() {
    if (!insertTrigger || !insertMenu) return;
    const r = insertTrigger.getBoundingClientRect();
    insertMenu.style.left = `${r.left}px`;
    insertMenu.style.top = `${r.bottom + 4}px`;
  }

  function insert(i: number) {
    insertMenu?.hidePopover();
    const r = insertSnippet(draft.text, SNIPPETS[i]);
    editor?.setText(r.text, r.from, r.to);
  }

  async function openLocal(file: File) {
    try {
      let text = await file.text();
      if (/\.json$/i.test(file.name)) text = stringify(JSON.parse(text));
      else if (!/\.ya?ml$/i.test(file.name)) {
        return toast.info("The editor opens YAML and JSON. Use Create workflow for JSON5 and KDL.");
      }
      draft.open(text);
    } catch (e) {
      toast.error(e);
    }
  }

  async function submitPath(e: SubmitEvent) {
    e.preventDefault();
    const path = pathInput.trim();
    if (!path) return;
    pathBusy = true;
    try {
      if (pathDialog === "open") {
        const r = await dashPost<{ success: boolean; content: string | null; error: string | null; exists: boolean }>(
          "/api/cli/read-file",
          { path },
        );
        if (!r.exists) return toast.error(`${path} does not exist`);
        if (!r.success || r.content === null) return toast.error(r.error ?? "Could not read the file");
        const text = /\.json$/i.test(path) ? stringify(parse(r.content)) : r.content;
        pathDialog = null; // before the replace confirmation, which is its own dialog
        await draft.open(text, /\.ya?ml$/i.test(path) ? path : null);
        return;
      } else if (!(await write(path, false))) return; // stay open to pick another path
      pathDialog = null;
    } catch (err) {
      toast.error(err);
    } finally {
      pathBusy = false;
    }
  }

  /** Save to `path`; asks before replacing an existing file unless it's the one we opened. */
  async function write(path: string, overwrite: boolean): Promise<boolean> {
    type Res = { success: boolean; path: string | null; error: string | null; exists: boolean };
    let r = await dashPost<Res>("/api/cli/write-file", { path, content: draft.text, overwrite });
    if (!r.success && r.exists && !overwrite) {
      if (!(await confirm({ title: `Replace ${path}?`, message: "The file already exists.", confirmLabel: "Replace" })))
        return false;
      r = await dashPost<Res>("/api/cli/write-file", { path, content: draft.text, overwrite: true });
    }
    if (!r.success) {
      toast.error(r.error ?? "Save failed");
      return false;
    }
    draft.path = r.path ?? path;
    draft.saved = draft.text;
    toast.success(`Saved ${draft.path}`);
    return true;
  }

  function save() {
    if (draft.path) write(draft.path, true).catch((e) => toast.error(e));
    else openPathDialog("save");
  }

  function openPathDialog(kind: "open" | "save") {
    pathInput = kind === "save" ? (draft.path ?? suggestName()) : "";
    pathDialog = kind;
  }

  function suggestName(): string {
    const name = /^name:\s*["']?([\w.-]+)/m.exec(draft.text)?.[1];
    return `${name ?? "workflow"}.yaml`;
  }

  async function validate() {
    validating = true;
    try {
      report = await validateSpec({ spec: draft.text, is_file: false, file_extension: ".yaml" });
    } catch (e) {
      toast.error(e);
    } finally {
      validating = false;
    }
  }

  async function create() {
    if (creating) return;
    creating = true;
    try {
      const id = await createWorkflow({ spec: draft.text, is_file: false, file_extension: ".yaml" }, null);
      toast.success(id !== null ? `Created workflow ${id}` : "Workflow created");
      app.refresh();
      if (id !== null) {
        if (initialize) await initializeWorkflow(id);
        app.go("workflows", id);
      }
    } catch (e) {
      report = { valid: false, errors: [errorText(e)], warnings: [] };
      toast.error("Failed to create workflow");
    } finally {
      creating = false;
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === "s" && !pathDialog) {
      e.preventDefault();
      save();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="page">
  <div class="toolbar">
    <button class="btn btn-sm" onclick={showTemplates}><Icon name="plus" size={13} /> New from template</button>
    <button
      bind:this={insertTrigger}
      class="btn btn-sm"
      popovertarget={insertId}
      onclick={placeInsertMenu}>Insert <Icon name="chevronDown" size={13} /></button
    >
    <div bind:this={insertMenu} id={insertId} popover="auto" class="menu" role="menu">
      {#each SNIPPETS as s, i (s.label)}
        <button role="menuitem" onclick={() => insert(i)}>
          {s.label}<span class="faint section">{s.section}</span>
        </button>
      {/each}
    </div>
    <button class="btn btn-sm" onclick={() => fileInput?.click()} title="Open a YAML or JSON file from this computer">
      <Icon name="upload" size={13} /> Open local file
    </button>
    <button class="btn btn-sm" onclick={() => openPathDialog("open")} title="Open a file on the machine running torc-dash">
      <Icon name="file" size={13} /> Open path
    </button>
    <button class="btn btn-sm" onclick={save} title="Save (⌘S / Ctrl+S)">
      <Icon name="download" size={13} /> Save
    </button>
    {#if draft.path}
      <button class="btn btn-sm btn-ghost" onclick={() => openPathDialog("save")}>Save as…</button>
    {/if}
    <span class="file muted" title={draft.path ?? ""}>
      {draft.path ?? "Unsaved draft"}{draft.dirty ? " •" : ""}
    </span>
    <span class="grow"></span>
    <label class="check"><input type="checkbox" bind:checked={initialize} /> Initialize after create</label>
    <button class="btn btn-sm" onclick={validate} disabled={validating}>
      {#if validating}<span class="spinner small"></span>{:else}<Icon name="check" size={13} />{/if} Validate
    </button>
    <button class="btn btn-sm btn-primary" onclick={create} disabled={creating || !draft.text.trim()}>
      {creating ? "Creating…" : "Create workflow"}
    </button>
  </div>
  <input
    bind:this={fileInput}
    type="file"
    accept=".yaml,.yml,.json"
    hidden
    onchange={(e) => {
      const f = e.currentTarget.files?.[0];
      e.currentTarget.value = "";
      if (f) openLocal(f);
    }}
  />

  <div class="body">
    {#await schema}
      <div class="empty"><span class="spinner"></span></div>
    {:then s}
      <SpecEditor bind:this={editor} bind:value={draft.text} schema={s} />
    {/await}
    {#if report}
      <aside>
        <ValidationPanel {report} busy={validating} />
        <button class="btn btn-sm btn-ghost" onclick={() => (report = null)}>Dismiss</button>
      </aside>
    {/if}
  </div>
  <p class="faint tips">
    Ctrl+Space suggests fields · hover a field for its docs · problems are underlined as you type ·
    Validate runs a full dry-run create
  </p>
</div>

<Modal bind:open={templatesOpen} title="New from template" size="xl">
  <div class="templates">
    <ul class="list">
      {#each allTemplates as t, i (t.name)}
        {#if t.group !== allTemplates[i - 1]?.group}
          <li class="group">{t.group}</li>
        {/if}
        <li>
          <button
            class:active={picked === t}
            onclick={() => (picked = t)}
            ondblclick={() => useTemplate(t.text)}
          >
            <strong>{t.name}</strong><span class="muted">{t.description}</span>
          </button>
        </li>
      {/each}
      {#if !templates}
        <li class="empty"><span class="spinner"></span></li>
      {/if}
    </ul>
    <!-- eslint-disable-next-line svelte/no-at-html-tags -- escaped by highlightYaml -->
    <pre class="preview code-block hl">{@html preview}</pre>
  </div>
  {#snippet footer()}
    <span class="faint grow">Double-click a template to use it</span>
    <button class="btn" onclick={() => (templatesOpen = false)}>Cancel</button>
    <button class="btn btn-primary" disabled={!picked} onclick={() => picked && useTemplate(picked.text)}>
      Use template
    </button>
  {/snippet}
</Modal>

<Modal
  open={pathDialog !== null}
  title={pathDialog === "open" ? "Open file" : "Save as"}
  size="md"
  onclose={() => (pathDialog = null)}
>
  <form id="path-form" onsubmit={submitPath}>
    <label class="field">
      Path on the machine running torc-dash
      <!-- svelte-ignore a11y_autofocus -->
      <input class="input" bind:value={pathInput} placeholder="workflow.yaml" autofocus />
      <span class="hint">Relative paths resolve against torc-dash's working directory.</span>
    </label>
  </form>
  {#snippet footer()}
    <button class="btn" onclick={() => (pathDialog = null)}>Cancel</button>
    <button class="btn btn-primary" type="submit" form="path-form" disabled={pathBusy || !pathInput.trim()}>
      {pathDialog === "open" ? "Open" : "Save"}
    </button>
  {/snippet}
</Modal>

<style>
  .page {
    gap: 10px;
  }
  .file {
    font-size: 12.5px;
    max-width: 320px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl; /* keep the file name visible when the path is long */
    text-align: left;
  }
  .body {
    flex: 1;
    min-height: 0;
    display: flex;
    gap: 12px;
  }
  .body > :global(.spec-editor) {
    flex: 1;
    min-width: 0;
  }
  aside {
    width: 320px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
    overflow: auto;
  }
  .tips {
    margin: 0;
    font-size: 12px;
  }
  .spinner.small {
    width: 12px;
    height: 12px;
  }
  .menu {
    position: fixed;
    inset: auto;
    margin: 0;
    min-width: 260px;
    padding: 4px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    color: var(--text);
    box-shadow: var(--shadow-lg);
  }
  .menu button {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: 30px;
    padding: 0 10px;
    border: none;
    border-radius: var(--radius-sm);
    background: none;
    color: inherit;
    font: inherit;
    font-size: 13px;
    text-align: left;
    cursor: pointer;
  }
  .menu button:hover,
  .menu button:focus-visible {
    background: var(--surface-hover);
    outline: none;
  }
  .section {
    margin-left: auto;
    font-family: var(--mono);
    font-size: 11.5px;
  }
  .templates {
    display: grid;
    grid-template-columns: minmax(260px, 360px) 1fr;
    gap: 16px;
    height: 100%;
    min-height: 0;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    min-height: 0;
    overflow: auto;
  }
  .list .group {
    padding: 12px 10px 4px;
    color: var(--text-muted);
    font-size: 11.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .list .group:first-child {
    padding-top: 0;
  }
  .list li button {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    padding: 8px 10px;
    border: none;
    border-radius: var(--radius-sm);
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .list li button:hover,
  .list li button:focus-visible {
    background: var(--surface-hover);
    outline: none;
  }
  .list li button.active {
    background: var(--accent-soft);
  }
  .list strong {
    font-family: var(--mono);
    font-size: 13px;
  }
  .list .muted {
    font-size: 12.5px;
  }
  .preview {
    margin: 0;
    min-height: 0;
    overflow: auto;
  }
</style>
